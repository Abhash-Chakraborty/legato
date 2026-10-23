package tech.abhashchakraborty.legato

import android.Manifest
import android.annotation.SuppressLint
import android.bluetooth.BluetoothDevice
import android.bluetooth.BluetoothManager
import android.bluetooth.BluetoothSocket
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import java.io.DataInputStream
import java.io.IOException
import java.io.InputStream
import java.io.OutputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.security.MessageDigest
import java.util.UUID
import kotlin.concurrent.thread

/**
 * Bluetooth to the PC, for when the phone and the PC share no network. The
 * core speaks the same TLS as over Wi-Fi to a socket on 127.0.0.1; this
 * carries that socket's bytes over RFCOMM to Legato's service on the PC.
 * Any app on the phone can reach 127.0.0.1, so only a connection that first
 * sends the core's one-time token is carried.
 */
object BluetoothLink {
    /** Legato's sync service on the PC; protocol::BT_SERVICE in the core. */
    private val SERVICE: UUID = UUID.fromString("d33e3e4c-9cb7-4638-b68d-813565eee1a7")

    /** Android asks to pair at most once a run; a dismissed prompt every five seconds would be a nag. */
    @Volatile private var askedToPair = false

    /** Android 12 and later ask before an app may use Bluetooth. */
    fun needsPermission(context: Context): Boolean =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
            context.checkSelfPermission(Manifest.permission.BLUETOOTH_CONNECT) != PackageManager.PERMISSION_GRANTED

    fun isOn(context: Context): Boolean =
        context.getSystemService(BluetoothManager::class.java)?.adapter?.isEnabled == true

    /** Connects to the PC at [address] and returns the local port that carries it for [token], or 0. */
    @SuppressLint("MissingPermission")
    fun open(context: Context, address: String, token: String): Int {
        if (needsPermission(context)) return 0
        val adapter = context.getSystemService(BluetoothManager::class.java)?.adapter ?: return 0
        if (!adapter.isEnabled) return 0
        val device = try {
            adapter.getRemoteDevice(address)
        } catch (e: IllegalArgumentException) {
            return 0
        }
        val remote = connect(device) ?: return 0
        val server = try {
            // 127.0.0.1, where the core dials: Android's getLoopbackAddress() is ::1.
            ServerSocket(0, 4, InetAddress.getByAddress(byteArrayOf(127, 0, 0, 1)))
        } catch (e: IOException) {
            runCatching { remote.close() }
            return 0
        }
        thread(name = "legato-bluetooth", isDaemon = true) {
            try {
                accept(server, token.toByteArray())?.use { local ->
                    local.tcpNoDelay = true
                    carry(local, remote)
                }
            } catch (_: IOException) {
            } finally {
                runCatching { server.close() }
                runCatching { remote.close() }
            }
        }
        return server.localPort
    }

    /** The first connection within 10 s that starts with [token]; others are closed. */
    private fun accept(server: ServerSocket, token: ByteArray): Socket? {
        val deadline = System.currentTimeMillis() + 10_000
        while (true) {
            val left = deadline - System.currentTimeMillis()
            if (left <= 0) return null
            server.soTimeout = left.toInt()
            val socket = server.accept()
            try {
                socket.soTimeout = 2_000
                val got = ByteArray(token.size)
                DataInputStream(socket.getInputStream()).readFully(got)
                if (MessageDigest.isEqual(got, token)) {
                    socket.soTimeout = 0
                    return socket
                }
            } catch (_: IOException) {
            }
            runCatching { socket.close() }
        }
    }

    /** A phone already paired with the PC connects at once; otherwise try without pairing, then let Android ask once. */
    @SuppressLint("MissingPermission")
    private fun connect(device: BluetoothDevice): BluetoothSocket? {
        val bonded = device.bondState == BluetoothDevice.BOND_BONDED
        val tries = when {
            bonded -> listOf(true)
            askedToPair -> listOf(false)
            else -> listOf(false, true)
        }
        for (secure in tries) {
            if (secure && !bonded) askedToPair = true
            val socket = try {
                if (secure) device.createRfcommSocketToServiceRecord(SERVICE)
                else device.createInsecureRfcommSocketToServiceRecord(SERVICE)
            } catch (e: IOException) {
                continue
            }
            try {
                socket.connect()
                return socket
            } catch (e: IOException) {
                runCatching { socket.close() }
            }
        }
        return null
    }

    /** Copies both ways until either side closes, then closes both. */
    private fun carry(local: Socket, remote: BluetoothSocket) {
        val up = thread(name = "legato-bluetooth-up", isDaemon = true) {
            copy(local.getInputStream(), remote.outputStream)
            runCatching { remote.close() }
        }
        copy(remote.inputStream, local.getOutputStream())
        runCatching { local.close() }
        up.join(2_000)
    }

    private fun copy(input: InputStream, output: OutputStream) {
        val buf = ByteArray(16 * 1024)
        try {
            while (true) {
                val n = input.read(buf)
                if (n < 0) break
                output.write(buf, 0, n)
                output.flush()
            }
        } catch (_: IOException) {
        }
    }
}
