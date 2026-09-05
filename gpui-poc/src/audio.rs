//! 音频：按键音效 + 语音播报（对应 renderer/src/services/click-sound.ts 与 voice.ts）
//!
//! - 按键音：实时合成一段 900Hz → 560Hz 快速衰减的"嗒"声，不依赖资源文件
//! - 语音：内置三段 mp3（输入提示 / 报告已查到 / 取走报告），可被
//!   app_data_dir/voice/ 下的同名配置文件覆盖；其余语音键在 Web 版本
//!   中也仅在内置语音缺失时静音，这里保持一致。
//!
//! rodio 的 OutputStream 不是 Send/Sync，因此独占专用线程持有输出流，
//! UI 线程经消息通道发送播放指令；播放失败一律静默（写日志）。

use std::io::Cursor;
use std::sync::OnceLock;
use std::sync::mpsc::{Sender, channel};

use rodio::buffer::SamplesBuffer;
use rodio::source::Source;
use rodio::{Decoder, OutputStream, Sink};

use crate::domain::config::AppConfig;
use crate::paths;

/// 语音键（对应 voice.ts VoiceKey）
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VoiceKey {
    Input,
    ReportsFound,
    PrintComplete,
}

/// 内置语音 mp3（随二进制分发，来自 resources/assets/voice/）
const INPUT_VOICE: &[u8] = include_bytes!("../resources/assets/voice/请您输入登记号病历号.mp3");
const REPORTS_FOUND_VOICE: &[u8] =
    include_bytes!("../resources/assets/voice/请您选择要打印的报告.mp3");
const PRINT_COMPLETE_VOICE: &[u8] = include_bytes!("../resources/assets/voice/请取走您的报告.mp3");

/// 播放指令
enum Msg {
    Click(f32),
    Voice(Vec<u8>, f32),
    StopVoice,
}

static TX: OnceLock<Sender<Msg>> = OnceLock::new();

/// 启动音频线程（失败只记日志）
pub fn init() {
    let (tx, rx) = channel::<Msg>();
    if TX.set(tx).is_err() {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("kiosk-audio".into())
        .spawn(move || run(rx));
    if spawned.is_err() {
        crate::domain::log::warn("audio", "音频线程启动失败");
    }
}

/// 音频线程主体：持有 OutputStream 与当前语音 Sink
fn run(rx: std::sync::mpsc::Receiver<Msg>) {
    let (_stream, handle) = match OutputStream::try_default() {
        Ok(pair) => pair,
        Err(e) => {
            crate::domain::log::warn("audio", &format!("音频输出初始化失败: {e}"));
            return;
        }
    };

    let mut voice_sink: Option<Sink> = None;
    while let Ok(msg) = rx.recv() {
        match msg {
            Msg::Click(volume) => {
                if volume > 0.0 {
                    let source = SamplesBuffer::new(1, 44100, click_samples(volume));
                    if let Err(e) = handle.play_raw(source) {
                        crate::domain::log::warn("audio", &format!("按键音播放失败: {e}"));
                    }
                }
            }
            Msg::Voice(bytes, volume) => {
                if let Some(old) = voice_sink.take() {
                    old.stop();
                }
                match Decoder::new(Cursor::new(bytes)) {
                    Ok(source) => match Sink::try_new(&handle) {
                        Ok(sink) => {
                            sink.append(source.amplify(volume));
                            voice_sink = Some(sink);
                        }
                        Err(e) => {
                            crate::domain::log::warn("audio", &format!("语音通道创建失败: {e}"))
                        }
                    },
                    Err(_) => crate::domain::log::warn("audio", "语音解码失败"),
                }
            }
            Msg::StopVoice => {
                if let Some(sink) = voice_sink.take() {
                    sink.stop();
                }
            }
        }
    }
}

fn send(msg: Msg) {
    if let Some(tx) = TX.get() {
        let _ = tx.send(msg);
    }
}

/// 播放按键音效，volume_percent 为 0-100 音量百分比
pub fn play_click(volume_percent: u32) {
    let volume = (volume_percent.min(100) as f32) / 100.0;
    send(Msg::Click(volume));
}

/// 合成按键音采样：三角形波 900Hz → 560Hz，4ms 起音 + 指数衰减
fn click_samples(volume: f32) -> Vec<f32> {
    const SAMPLE_RATE: f32 = 44100.0;
    const PEAK: f32 = 0.28;
    let mut phase = 0.0f32;
    (0..(SAMPLE_RATE as usize * 110 / 1000))
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            let freq = 900.0 + (560.0 - 900.0) * (t / 0.055).min(1.0);
            phase += freq / SAMPLE_RATE;
            let wave = 4.0 * (phase.fract() - 0.5).abs() - 1.0; // 三角波
            let env = if t < 0.004 {
                t / 0.004
            } else {
                (-(t - 0.004) / 0.045).exp()
            };
            wave * PEAK * env * volume
        })
        .collect()
}

/// 播放语音（可被下一次播报打断）
pub fn speak(key: VoiceKey, config: &AppConfig) {
    if !config.terminal.voice_enabled {
        return;
    }
    let Some(bytes) = resolve_voice(key, config) else {
        return;
    };
    let volume = (config.terminal.voice_volume.clamp(0, 100) as f32) / 100.0;
    send(Msg::Voice(bytes, volume));
}

/// 停止当前语音播报
pub fn stop_speaking() {
    send(Msg::StopVoice);
}

/// 解析语音来源：优先 app_data_dir/voice/ 下的配置文件，否则回退内置语音
fn resolve_voice(key: VoiceKey, config: &AppConfig) -> Option<Vec<u8>> {
    let (field, embedded): (&str, &[u8]) = match key {
        VoiceKey::Input => (config.terminal.voice_input.trim(), INPUT_VOICE),
        VoiceKey::ReportsFound => (
            config.terminal.voice_reports_found.trim(),
            REPORTS_FOUND_VOICE,
        ),
        VoiceKey::PrintComplete => (
            config.terminal.voice_print_complete.trim(),
            PRINT_COMPLETE_VOICE,
        ),
    };
    if !field.is_empty() {
        let path = paths::app_data_dir().join("voice").join(field);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
        crate::domain::log::warn("audio", &format!("自定义语音读取失败，回退内置: {field}"));
    }
    Some(embedded.to_vec())
}
