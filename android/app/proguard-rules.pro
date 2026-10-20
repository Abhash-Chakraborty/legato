# UniFFI bindings reach the Rust library through JNA by name.
-keep class com.sun.jna.** { *; }
-keep class tech.abhashchakraborty.legato.core.** { *; }
-dontwarn java.awt.*
