package tech.abhashchakraborty.legato

import java.util.Locale

/**
 * A speech model this device can download and run: an entry from Legato's
 * catalog on the PC (`src-tauri/src/catalog/catalog.json`), pinned to one
 * revision and checked against its SHA-256 after the download.
 */
data class PhoneModel(
    val id: String,
    val name: String,
    val blurb: String,
    val repo: String,
    val revision: String,
    val file: String,
    val bytes: Long,
    val sha256: String,
    val languages: List<String>,
    /** Tells languages apart on its own; the others are told which to expect. */
    val detects: Boolean,
) {
    val url: String get() = "https://huggingface.co/$repo/resolve/$revision/$file"

    /** "77 MB". */
    val size: String get() = if (bytes >= 1_000_000_000) "%.1f GB".format(bytes / 1e9) else "${bytes / 1_000_000} MB"

    /**
     * The language to tell the model: the phone's own when the model knows
     * it, else the model's first; none when the model works it out itself.
     */
    fun language(locale: Locale = Locale.getDefault()): String? = when {
        detects -> null
        locale.language in languages -> locale.language
        else -> languages.firstOrNull()
    }
}

/** Small to large: each step costs more space, memory and battery for better text. */
object PhoneModels {
    const val DEFAULT = "parakeet-tdt_ctc-110m"

    /** From this size up a model is one of the heavier, more accurate ones. */
    const val HEAVY_BYTES = 400_000_000L

    fun byId(id: String?): PhoneModel = all.firstOrNull { it.id == id } ?: all.first { it.id == DEFAULT }

    val all = listOf(
        PhoneModel(
            id = "moonshine-tiny",
            name = "Moonshine Tiny",
            blurb = "English. The smallest and quickest, and the lightest on the battery; makes the most mistakes.",
            repo = "handy-computer/moonshine-tiny-gguf",
            revision = "f5c11906eba3f44cf305eed30feb9cbfb0b4b9d0",
            file = "moonshine-tiny-Q8_0.gguf",
            bytes = 35466912L,
            sha256 = "2fd348d7b38f97d309cc3ec6848f3f57f537b80244950f07d2637e463f95a3a1",
            languages = listOf("en"),
            detects = false,
        ),
        PhoneModel(
            id = "moonshine-base",
            name = "Moonshine Base",
            blurb = "English. Quick, and more accurate than Tiny.",
            repo = "handy-computer/moonshine-base-gguf",
            revision = "3ef112378a8cf46ac8b278d9bfa2d15c846704b8",
            file = "moonshine-base-Q8_0.gguf",
            bytes = 77476480L,
            sha256 = "7f0027dfd857d310b63a85ef57cadf183da712cc374f85a648f8bc18aaa2efc8",
            languages = listOf("en"),
            detects = false,
        ),
        PhoneModel(
            id = "whisper-base",
            name = "Whisper Base",
            blurb = "99 languages, Hindi among them, told apart on its own. Light and quick, but less accurate than the English-only models.",
            repo = "handy-computer/whisper-base-gguf",
            revision = "e0f69524f648720eca44c024d1d0dbb7027d1fa0",
            file = "whisper-base-Q8_0.gguf",
            bytes = 84962880L,
            sha256 = "81c069428bc8a24551a8169cf31cf09bcfd9d4cf50389ae281323c9aa9648c81",
            languages = listOf("en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo", "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln", "ha", "ba", "jw", "su"),
            detects = true,
        ),
        PhoneModel(
            id = "parakeet-tdt_ctc-110m",
            name = "Parakeet 110M",
            blurb = "English. As quick as Moonshine Base and clearly more accurate: the best light choice for English.",
            repo = "handy-computer/parakeet-tdt_ctc-110m-gguf",
            revision = "9d66d34f9e1594075c5dd72c90c0f4c321b29f21",
            file = "parakeet-tdt_ctc-110m-Q8_0.gguf",
            bytes = 135373280L,
            sha256 = "7dd44c74a331d788a4e5f8b16913b3feb29ced22cf5613aad0e0f6cd30516296",
            languages = listOf("en"),
            detects = false,
        ),
        PhoneModel(
            id = "moonshine-streaming-small",
            name = "Moonshine Small",
            blurb = "English. Quick, and close to Parakeet 110M.",
            repo = "handy-computer/moonshine-streaming-small-gguf",
            revision = "41444173ed8210852a883e046fadcfba3e7bfbae",
            file = "moonshine-streaming-small-Q8_0.gguf",
            bytes = 198506848L,
            sha256 = "d03670f69629b649085d0f44a63d97668b4119117cc9611a4e4ad94341713dfc",
            languages = listOf("en"),
            detects = false,
        ),
        PhoneModel(
            id = "canary-180m-flash",
            name = "Canary 180M Flash",
            blurb = "English, German, Spanish and French. Quick, and more accurate than Parakeet 110M.",
            repo = "handy-computer/canary-180m-flash-gguf",
            revision = "b147f9dc52b59f0998e410540a84727bd86457fd",
            file = "canary-180m-flash-Q8_0.gguf",
            bytes = 218447552L,
            sha256 = "e13c7f5d0952b056a027cfffec13e3a3a134d1608babed24f983568f141e297c",
            languages = listOf("en", "de", "es", "fr"),
            detects = false,
        ),
        PhoneModel(
            id = "whisper-small",
            name = "Whisper Small",
            blurb = "99 languages, Hindi among them, told apart on its own. Slower.",
            repo = "handy-computer/whisper-small-gguf",
            revision = "c0214bd34be9296695486f838e0142f900803159",
            file = "whisper-small-Q8_0.gguf",
            bytes = 269751136L,
            sha256 = "9b9c8811bbcc82a7766f0fb0925614bdacb0923b2cc630daeac17108b655b860",
            languages = listOf("en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar", "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta", "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn", "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo", "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln", "ha", "ba", "jw", "su"),
            detects = true,
        ),
        PhoneModel(
            id = "moonshine-streaming-medium",
            name = "Moonshine Medium",
            blurb = "English. Accurate and still fairly quick.",
            repo = "handy-computer/moonshine-streaming-medium-gguf",
            revision = "c722a9455a40a1844c3d25267dc84eff61d8dd84",
            file = "moonshine-streaming-medium-Q8_0.gguf",
            bytes = 295793568L,
            sha256 = "f7c9564249b508f6012927ec4f9e536087da53a7047f858ca9975bea5f75299e",
            languages = listOf("en"),
            detects = false,
        ),
        PhoneModel(
            id = "parakeet-tdt-0.6b-v2",
            name = "Parakeet 0.6B v2",
            blurb = "English. Very accurate, and quick for its size.",
            repo = "handy-computer/parakeet-tdt-0.6b-v2-gguf",
            revision = "07cee0616125a08ef619729bb47f40ef747e4bc4",
            file = "parakeet-tdt-0.6b-v2-Q4_K_M.gguf",
            bytes = 475491840L,
            sha256 = "4853f9653f641d376e6f7de65d73c7a34a73677704a606727bf51acc83f999f3",
            languages = listOf("en"),
            detects = false,
        ),
        PhoneModel(
            id = "parakeet-tdt-0.6b-v3",
            name = "Parakeet 0.6B v3",
            blurb = "25 European languages, told apart on its own. Nearly as accurate as v2 in English.",
            repo = "handy-computer/parakeet-tdt-0.6b-v3-gguf",
            revision = "85ac09ea12fc4b1112fa76810059364bc6adc9de",
            file = "parakeet-tdt-0.6b-v3-Q4_K_M.gguf",
            bytes = 485425504L,
            sha256 = "b68557be1e3c40207fd7c4bd9d63f1d3316b963f15325bfb0cc16a8bb0ffd181",
            languages = listOf("bg", "hr", "cs", "da", "nl", "en", "et", "fi", "fr", "de", "el", "hu", "it", "lv", "lt", "mt", "pl", "pt", "ro", "ru", "sk", "sl", "es", "sv", "uk"),
            detects = true,
        ),
        PhoneModel(
            id = "qwen3-asr-0.6b",
            name = "Qwen3-ASR 0.6B",
            blurb = "30 languages, Hindi among them, told apart on its own. Accurate, slower than Parakeet.",
            repo = "handy-computer/Qwen3-ASR-0.6B-gguf",
            revision = "e4e16599b900eb0cb36e524514756bb92eb092b7",
            file = "Qwen3-ASR-0.6B-Q4_K_M.gguf",
            bytes = 589560480L,
            sha256 = "5b58f32a58ffa2c8783e0b0963485623e286e6272d953dfc9e28bc3447dee0c0",
            languages = listOf("zh", "en", "yue", "ar", "de", "fr", "es", "pt", "id", "it", "ko", "ru", "th", "vi", "ja", "tr", "hi", "ms", "nl", "sv", "da", "fi", "pl", "cs", "fil", "fa", "el", "ro", "hu", "mk"),
            detects = true,
        ),
        PhoneModel(
            id = "canary-1b-flash",
            name = "Canary 1B Flash",
            blurb = "English, German, Spanish and French. The most accurate here, and the heaviest on memory and battery.",
            repo = "handy-computer/canary-1b-flash-gguf",
            revision = "b427664769b93c021df108a2fa8bfb858ae236c1",
            file = "canary-1b-flash-Q4_K_M.gguf",
            bytes = 677141280L,
            sha256 = "2521a615a2b04ab11900d894e1f5c5a70405d85aa74be0806b569b9cc311a707",
            languages = listOf("en", "de", "es", "fr"),
            detects = false,
        ),
    )
}
