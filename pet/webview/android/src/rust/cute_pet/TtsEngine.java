package rust.cute_pet;

import android.content.Context;
import android.media.AudioAttributes;
import android.media.AudioFormat;
import android.media.AudioManager;
import android.media.AudioTrack;
import android.os.SystemClock;
import android.util.Log;

import com.k2fsa.sherpa.onnx.GeneratedAudio;
import com.k2fsa.sherpa.onnx.OfflineTts;
import com.k2fsa.sherpa.onnx.OfflineTtsConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsKittenModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsKokoroModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsMatchaModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsPocketModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsSupertonicModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsVitsModelConfig;
import com.k2fsa.sherpa.onnx.OfflineTtsZipVoiceModelConfig;

import java.util.concurrent.atomic.AtomicInteger;
import java.util.concurrent.atomic.AtomicReference;

/**
 * 壳内离线中文 TTS(sherpa-onnx VITS)。
 *
 * 设计:
 *  - 模型**直接从 APK assets 读**(OfflineTts(AssetManager, config)), 不拷 filesDir、
 *    无网络、无外部依赖 —— 单机可用(需求: "手机单机也能说中文")。
 *  - 引擎 init(加载 onnx + lexicon, ~1-3s)在 worker 线程做, JS 侧 speak() 提前到来时
 *    只是入队等 init 完, 不阻塞 WebView 桥线程。
 *  - 单 worker 串行: 同一时刻只合成/播放一条; speak() 新消息**顶掉**旧 pending,
 *    正在播的旧音频也会被打断(桌宠说话天然"新的盖旧的")。
 *  - 模型只带基础件(model.onnx/lexicon/tokens/date.fst/number.fst);
 *    仓库里的 rule.far 是 180MB 的 jieba 大词典, **绝不进 APK**(体积)。
 *
 * 后续(第二步): 换 ZipVoice int8 + 丛雨参考音 —— 参考音走用户自备素材目录
 * (合规: 角色素材不进包, 见 THIRD-PARTY-NOTICES.md §2.2), 1.13.8 已内置
 * OfflineTtsZipVoiceModelConfig, 运行时无需升级。
 */
public final class TtsEngine {

    private static final String TAG = "CutePetTts";

    /** assets 下模型目录(build.sh 负责把模型文件放这里) */
    private static final String MODEL_DIR = "pet/tts/vits-icefall-zh-aishell3";

    private final Context ctx;
    private final int defaultSid;

    private final Thread worker;
    private final Object lock = new Object();
    private final AtomicReference<String> pending = new AtomicReference<>(null);
    private final AtomicInteger playGen = new AtomicInteger(0); // 播放代次: ++ 即打断当前播放

    private volatile OfflineTts tts;
    private volatile boolean failed;
    private volatile boolean released;

    public TtsEngine(Context ctx, int defaultSid) {
        this.ctx = ctx.getApplicationContext();
        this.defaultSid = defaultSid;
        worker = new Thread(this::run, "cute-pet-tts");
        worker.setPriority(Thread.NORM_PRIORITY - 1); // 别抢渲染/输入线程
        worker.start();
    }

    /** JS 侧入口: 说话(非阻塞)。新请求顶掉旧 pending 与正在播放的旧音频。 */
    public void speak(String text) {
        if (failed || released) {
            return;
        }
        if (text == null || (text = text.trim()).isEmpty()) {
            return;
        }
        playGen.incrementAndGet(); // 打断当前播放
        pending.set(text);
        synchronized (lock) {
            lock.notifyAll();
        }
    }

    /** 停止当前播放并清空待说队列 */
    public void stop() {
        pending.set(null);
        playGen.incrementAndGet();
    }

    /** 服务销毁: 停播放、停线程。native 合成中途无法打断, worker 会在当次结束后退出。 */
    public void release() {
        released = true;
        pending.set(null);
        playGen.incrementAndGet();
        synchronized (lock) {
            lock.notifyAll();
        }
    }

    // ---------------- worker ----------------

    private void run() {
        try {
            long t0 = SystemClock.elapsedRealtime();
            OfflineTtsVitsModelConfig vits = new OfflineTtsVitsModelConfig(
                    MODEL_DIR + "/model.onnx",
                    MODEL_DIR + "/lexicon.txt",
                    MODEL_DIR + "/tokens.txt",
                    "",     // dataDir
                    "",     // dictDir
                    0.667f, // noiseScale
                    0.8f,   // noiseScaleW
                    1.0f);  // lengthScale
            // Kotlin data class 的空参实例 == Kotlin 侧默认值(不能用 null, Intrinsics 会拦)
            OfflineTtsModelConfig model = new OfflineTtsModelConfig(
                    vits,
                    new OfflineTtsMatchaModelConfig(),
                    new OfflineTtsKokoroModelConfig(),
                    new OfflineTtsZipVoiceModelConfig(),
                    new OfflineTtsKittenModelConfig(),
                    new OfflineTtsPocketModelConfig(),
                    new OfflineTtsSupertonicModelConfig(),
                    2,      // numThreads: 手机大核 2 个, 多了反而抢
                    false,  // debug
                    "cpu");
            OfflineTtsConfig cfg = new OfflineTtsConfig(
                    model,
                    MODEL_DIR + "/date.fst," + MODEL_DIR + "/number.fst", // ruleFsts: 数字/日期读法
                    "",     // ruleFars(180MB jieba 词典, 不进包)
                    1,      // maxNumSentences
                    1.0f);  // silenceScale
            tts = new OfflineTts(ctx.getAssets(), cfg);
            Log.i(TAG, "init ok in " + (SystemClock.elapsedRealtime() - t0) + "ms, "
                    + "sr=" + tts.sampleRate() + " speakers=" + tts.numSpeakers());
        } catch (Throwable t) {
            failed = true;
            Log.e(TAG, "init failed(TTS 将静默禁用): " + t);
            return;
        }

        while (!released) {
            String text;
            synchronized (lock) {
                while (!released && pending.get() == null) {
                    try {
                        lock.wait();
                    } catch (InterruptedException e) {
                        return;
                    }
                }
                if (released) {
                    return;
                }
                text = pending.getAndSet(null);
            }
            if (text == null) {
                continue;
            }
            try {
                long t1 = SystemClock.elapsedRealtime();
                int sid = defaultSid;
                // 线程注释: 合成是 CPU 密集; 期间新 speak 会 ++playGen, 播放阶段据此打断
                GeneratedAudio audio = tts.generate(text, sid, 1.0f);
                Log.i(TAG, "generate \"" + preview(text) + "\" "
                        + (SystemClock.elapsedRealtime() - t1) + "ms, "
                        + (audio.getSamples() == null ? 0 : audio.getSamples().length) + " samples @"
                        + audio.getSampleRate());
                if (audio.getSamples() != null && audio.getSamples().length > 0) {
                    play(audio);
                }
            } catch (Throwable t) {
                Log.e(TAG, "generate/play failed: " + t);
            }
        }
    }

    /** 分块流式播放, 每 chunk 检查 playGen(被新 speak/stop 顶掉则立即停) */
    private void play(GeneratedAudio audio) {
        int myGen = playGen.get();
        float[] s = audio.getSamples();
        int sr = audio.getSampleRate();
        AudioTrack track = null;
        try {
            AudioAttributes attrs = new AudioAttributes.Builder()
                    .setUsage(AudioAttributes.USAGE_ASSISTANT)
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build();
            AudioFormat fmt = new AudioFormat.Builder()
                    .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                    .setSampleRate(sr)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .build();
            int minBuf = AudioTrack.getMinBufferSize(sr, AudioFormat.CHANNEL_OUT_MONO,
                    AudioFormat.ENCODING_PCM_FLOAT);
            track = new AudioTrack(attrs, fmt, Math.max(minBuf * 4, 65536),
                    AudioTrack.MODE_STREAM, AudioManager.AUDIO_SESSION_ID_GENERATE);
            track.play();
            int chunk = 4096;
            for (int off = 0; off < s.length; off += chunk) {
                if (released || playGen.get() != myGen) {
                    break; // 被新请求/停止打断
                }
                int n = Math.min(chunk, s.length - off);
                int w = track.write(s, off, n, AudioTrack.WRITE_BLOCKING);
                if (w < 0) {
                    break;
                }
            }
        } catch (Throwable t) {
            Log.w(TAG, "play failed: " + t);
        } finally {
            if (track != null) {
                try {
                    track.stop();
                } catch (Exception ignored) {
                }
                try {
                    track.release();
                } catch (Exception ignored) {
                }
            }
        }
    }

    private static String preview(String s) {
        return s.length() > 24 ? s.substring(0, 24) + "…" : s;
    }
}
