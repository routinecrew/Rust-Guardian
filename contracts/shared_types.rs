// ============================================================
// Rust-Guardian 공유 계약 (Shared Contracts)
// ============================================================
// 모든 에이전트는 이 파일의 타입과 trait을 기준으로 개발한다.
// 이 파일을 수정하려면 반드시 모든 에이전트에게 알려야 한다.
// ============================================================

// ----- 의존성 -----
// bytes = "1"
// tokio = { version = "1", features = ["full"] }
// serde = { version = "1", features = ["derive"] }
// async-trait = "0.1"
// thiserror = "1"
// chrono = { version = "0.4", features = ["serde"] }
// ed25519-dalek = "2"

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};

// ============================================================
// 1. SecureBytes — 원본 프레임 보호 래퍼
// ============================================================

/// 원본 데이터를 안전하게 관리하는 메모리 래퍼.
/// 비식별화 완료 후 즉시 `purge()`를 호출하여 원본을 삭제한다.
/// Drop 시에도 자동으로 zeroize된다.
pub struct SecureBytes {
    inner: Box<[u8]>,
    purged: AtomicBool,
}

impl SecureBytes {
    /// 원시 데이터로부터 SecureBytes를 생성한다.
    pub fn from_raw(data: Vec<u8>) -> Self {
        Self {
            inner: data.into_boxed_slice(),
            purged: AtomicBool::new(false),
        }
    }

    /// 읽기 전용 뷰를 반환한다. purge 후에는 None을 반환한다.
    pub fn view(&self) -> Option<&[u8]> {
        if self.purged.load(Ordering::Acquire) {
            None
        } else {
            Some(&self.inner)
        }
    }

    /// 데이터 길이를 반환한다.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 데이터가 비어있는지 확인한다.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// 이미 purge되었는지 확인한다.
    pub fn is_purged(&self) -> bool {
        self.purged.load(Ordering::Acquire)
    }

    /// 원본 데이터를 0으로 덮어쓰고 접근을 차단한다.
    /// 비식별화 완료 후 즉시 호출해야 한다.
    pub fn purge(&self) {
        if !self.purged.swap(true, Ordering::AcqRel) {
            // SAFETY: 단일 소유자(Box)이므로 안전하게 zeroize 가능.
            // purged 플래그로 이후 접근을 차단하므로 data race 없음.
            unsafe {
                let ptr = self.inner.as_ptr() as *mut u8;
                core::ptr::write_bytes(ptr, 0, self.inner.len());
            }
            std::sync::atomic::fence(Ordering::SeqCst);
        }
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        self.purge();
    }
}

// SecureBytes는 Send + Sync를 수동 구현 (AtomicBool로 동기화)
// SAFETY: inner는 Box 소유이고, purged는 AtomicBool이므로 thread-safe.
unsafe impl Send for SecureBytes {}
unsafe impl Sync for SecureBytes {}

// ============================================================
// 2. RgbFrame — 디코딩된 RGB 프레임
// ============================================================

/// 디코딩된 RGB 프레임 데이터.
#[derive(Clone, Debug)]
pub struct RgbFrame {
    pub width: u32,
    pub height: u32,
    /// RGB24 형식 (width * height * 3 바이트)
    pub data: Vec<u8>,
}

// ============================================================
// 3. Detection — 탐지 결과
// ============================================================

/// 바운딩 박스 (정규화 좌표 0.0 ~ 1.0)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// 단일 탐지 결과
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Detection {
    pub label: String,
    pub confidence: f32,
    pub bbox: BoundingBox,
    pub track_id: Option<u64>,
}

// ============================================================
// 4. AnonymizationLevel — 비식별화 등급
// ============================================================

/// GDPR 대응 비식별화 5단계 등급
#[derive(Clone, Debug, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnonymizationLevel {
    /// Level 1: Gaussian Blur — 복원 가능, GDPR 적용 대상
    Level1GaussianBlur = 1,
    /// Level 2: Strong Mosaic — 복원 어려움, GDPR 적용 대상
    Level2StrongMosaic = 2,
    /// Level 3: Solid Box — 원본 소멸, 익명화에 가까움
    Level3SolidBox = 3,
    /// Level 4: Synthetic Replacement — 합성 대체 (법률 자문 필수)
    Level4Synthetic = 4,
    /// Level 5: Complete Removal — 영역 완전 제거
    Level5Removal = 5,
}

// ============================================================
// 5. PrivacySignature — 디지털 서명
// ============================================================

/// 체인 상태 — 끊김 시 복구 지원
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChainStatus {
    /// 정상 연속 체인
    Normal,
    /// 체인 재시작 (정전/crash 복구)
    ChainReset {
        reason: ChainResetReason,
        previous_chain_last_seq: u64,
        previous_chain_last_hash: [u8; 32],
    },
}

/// 체인 리셋 사유
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChainResetReason {
    ProcessRestart,
    PowerFailure,
    ManualReset,
    KeyRotation,
}

/// 프라이버시 서명 (프레임당 1개)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacySignature {
    pub device_id: String,
    pub timestamp: u64,
    pub sequence: u64,
    pub prev_frame_hash: [u8; 32],
    pub nonce: [u8; 16],
    pub frame_hash: [u8; 32],
    pub detection_summary: String,
    pub model_version: String,
    pub policy_hash: [u8; 32],
    pub anonymization_level: u8,
    pub chain_status: ChainStatus,
    pub signature_bytes: Vec<u8>,
}

// ============================================================
// 6. ProcessedFrame — 비식별화 완료 프레임
// ============================================================

/// 비식별화 처리가 완료된 프레임.
/// 원본 데이터는 포함하지 않는다 (SecureBytes에서 purge됨).
#[derive(Clone, Debug)]
pub struct ProcessedFrame {
    /// 비식별화된 인코딩 데이터
    pub masked_payload: Vec<u8>,
    /// 탐지 결과 목록
    pub detections: Vec<Detection>,
    /// 적용된 비식별화 등급
    pub anonymization_level: AnonymizationLevel,
    /// 디지털 서명 (선택)
    pub signature: Option<PrivacySignature>,
    /// 프레임 시퀀스 번호
    pub sequence: u64,
    /// 타임스탬프 (밀리초)
    pub timestamp_ms: u64,
}

// ============================================================
// 7. Trait: InferenceBackend — 추론 엔진 추상화
// ============================================================

/// 하드웨어별 추론 백엔드 추상화.
/// 각 백엔드(ORT, NCNN, TFLite, TensorRT)가 이 trait을 구현한다.
#[async_trait]
pub trait InferenceBackend: Send + Sync {
    /// RGB 프레임에서 객체를 탐지한다.
    async fn infer(&self, frame: &RgbFrame) -> anyhow::Result<Vec<Detection>>;
    /// 백엔드 이름을 반환한다 (로그용).
    fn name(&self) -> &str;
}

// ============================================================
// 8. Trait: Masker — 비식별화 필터 추상화
// ============================================================

/// 비식별화 필터 추상화.
/// 등급별 마스킹 전략을 구현한다.
pub trait Masker: Send + Sync {
    /// 탐지된 영역에 비식별화를 적용한다.
    fn apply(
        &self,
        frame: &RgbFrame,
        detections: &[Detection],
        level: AnonymizationLevel,
    ) -> anyhow::Result<RgbFrame>;
}

// ============================================================
// 9. Trait: Signer — 서명 추상화
// ============================================================

/// 디지털 서명 추상화.
#[async_trait]
pub trait Signer: Send + Sync {
    /// 비식별화된 데이터와 탐지 결과에 서명한다.
    async fn sign(
        &self,
        masked_data: &[u8],
        detections: &[Detection],
        anonymization_level: AnonymizationLevel,
    ) -> anyhow::Result<PrivacySignature>;
}

// ============================================================
// 10. Trait: CaptureSource — 입력 소스 추상화
// ============================================================

/// 입력 소스 추상화 (V4L2, RTSP, File).
#[async_trait]
pub trait CaptureSource: Send + Sync {
    /// 다음 프레임을 가져온다.
    async fn next_frame(&mut self) -> anyhow::Result<Option<SecureBytes>>;
    /// 소스 이름을 반환한다.
    fn name(&self) -> &str;
    /// 소스를 중지한다.
    async fn stop(&mut self) -> anyhow::Result<()>;
}

// ============================================================
// 11. Trait: OutputSink — 출력 대상 추상화
// ============================================================

/// 출력 대상 추상화 (파일, RTSP, mTLS 전송 등).
#[async_trait]
pub trait OutputSink: Send + Sync {
    /// 처리된 프레임을 출력한다.
    async fn write(&self, frame: &ProcessedFrame) -> anyhow::Result<()>;
    /// 출력 대상 이름을 반환한다.
    fn name(&self) -> &str;
    /// 출력을 중지한다.
    async fn stop(&self) -> anyhow::Result<()>;
}

// ============================================================
// 12. Event Bus — 시스템 이벤트
// ============================================================

/// 시스템 전역 이벤트
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GuardianEvent {
    /// 탐지 이벤트
    Detection {
        timestamp_ms: u64,
        detections: Vec<Detection>,
        source: String,
    },
    /// 비식별화 완료
    FrameProcessed {
        timestamp_ms: u64,
        sequence: u64,
        detection_count: usize,
    },
    /// 서명 생성
    SignatureCreated {
        timestamp_ms: u64,
        sequence: u64,
    },
    /// 체인 리셋
    ChainReset {
        reason: ChainResetReason,
        previous_last_seq: u64,
    },
    /// 정확도 경고
    AccuracyWarning {
        actual_recall: f32,
        required_recall: f32,
    },
    /// 열 경고
    ThermalWarning {
        temperature: f32,
        action: String,
    },
    /// OTA 업데이트
    OtaUpdate {
        target: String,
        version: String,
        status: String,
    },
    /// 정책 변경
    PolicyChanged {
        policy_name: String,
        policy_hash: [u8; 32],
    },
    /// 감사 로그
    AuditLog {
        timestamp_ms: u64,
        action: String,
        details: String,
    },
    /// 시스템 상태
    SystemStatus {
        fps: f32,
        cpu_usage: f32,
        memory_mb: f32,
        temperature: Option<f32>,
    },
}

/// 이벤트 버스 타입
pub type EventSender = broadcast::Sender<GuardianEvent>;
pub type EventReceiver = broadcast::Receiver<GuardianEvent>;

/// 이벤트 버스 생성 (기본 버퍼: 1024)
pub fn new_event_bus() -> (EventSender, EventReceiver) {
    broadcast::channel(1024)
}

/// 프레임 파이프라인 채널 (mpsc — backpressure 지원)
pub type FrameSender = mpsc::Sender<SecureBytes>;
pub type FrameReceiver = mpsc::Receiver<SecureBytes>;

/// 프레임 채널 생성 (기본 버퍼: 32)
pub fn new_frame_channel(buffer: usize) -> (FrameSender, FrameReceiver) {
    mpsc::channel(buffer)
}

// ============================================================
// 13. Trait: PipelineStage — 파이프라인 단계 추상화
// ============================================================

/// 파이프라인의 각 단계를 추상화한다.
/// Core의 Pipeline이 이 trait을 통해 단계들을 조합한다.
#[async_trait]
pub trait PipelineStage: Send + Sync {
    /// 단계 이름
    fn name(&self) -> &str;
    /// 단계가 활성화되어 있는지
    fn is_enabled(&self) -> bool;
}

// ============================================================
// 14. Config 타입 — 설정 파일 구조
// ============================================================

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GuardianConfig {
    pub capture: CaptureConfig,
    pub detector: DetectorConfig,
    pub masker: MaskerConfig,
    pub signer: Option<SignerConfig>,
    pub audio: Option<AudioConfig>,
    pub output: OutputConfig,
    pub api: Option<ApiConfig>,
    pub thermal: Option<ThermalConfig>,
    pub ota: Option<OtaConfig>,
    pub audit: Option<AuditConfig>,
    pub agent: Option<AgentPolicyConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CaptureConfig {
    pub source: CaptureSourceType,
    pub path: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum CaptureSourceType {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "v4l2")]
    V4l2,
    #[serde(rename = "rtsp")]
    Rtsp,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DetectorConfig {
    pub model_path: String,
    pub backend: String,
    pub input_width: u32,
    pub input_height: u32,
    pub confidence_threshold: f32,
    pub nms_threshold: f32,
    pub target_labels: Vec<String>,
    pub quantization: Option<QuantizationConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QuantizationConfig {
    pub level: String,
    pub calibration_data: Option<String>,
    pub max_map50_loss: f32,
    pub max_recall_loss: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MaskerConfig {
    pub level: u8,
    pub legal_review_required: Option<bool>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignerConfig {
    pub enabled: bool,
    pub device_id: String,
    pub key_path: String,
    pub chain_state_path: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AudioConfig {
    pub enabled: bool,
    pub method: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OutputConfig {
    pub sinks: Vec<OutputSinkConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OutputSinkConfig {
    pub sink_type: String,
    pub path: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub address: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ThermalConfig {
    pub enabled: bool,
    pub warning_threshold: f32,
    pub critical_threshold: f32,
    pub check_interval: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct OtaConfig {
    pub enabled: bool,
    pub server_url: String,
    pub check_interval: u64,
    pub auto_apply_model: bool,
    pub auto_apply_policy: bool,
    pub auto_apply_binary: bool,
    pub rollback_timeout: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuditConfig {
    pub enabled: bool,
    pub log_path: String,
    pub retention_days: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AgentPolicyConfig {
    pub enabled: bool,
    pub policy_dir: String,
    pub sentry_mode: Option<SentryConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SentryConfig {
    pub enabled: bool,
    pub idle_fps: u32,
    pub active_fps: u32,
    pub activation_threshold: f32,
}

// ============================================================
// 15. Error 타입
// ============================================================

#[derive(thiserror::Error, Debug)]
pub enum GuardianError {
    #[error("capture failed: {0}")]
    Capture(String),
    #[error("detection failed: {0}")]
    Detection(String),
    #[error("masking failed: {0}")]
    Masking(String),
    #[error("signing failed: {0}")]
    Signing(String),
    #[error("purge failed: frame already purged")]
    Purge,
    #[error("thermal critical: {0}°C")]
    ThermalCritical(f32),
    #[error("config error: {0}")]
    Config(String),
    #[error("output error: {0}")]
    Output(String),
    #[error("audit error: {0}")]
    Audit(String),
    #[error("ota error: {0}")]
    Ota(String),
    #[error("policy error: {0}")]
    Policy(String),
}

// ============================================================
// 16. Hardware Detection
// ============================================================

/// 감지된 하드웨어 유형
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Hardware {
    JetsonOrin,
    JetsonNano,
    RpiWithTpu,
    Rpi5,
    Rpi4,
    X86WithCuda,
    X86Cpu,
    Unknown,
}

// ============================================================
// 17. FailSafe 정책
// ============================================================

/// 탐지 실패 시 안전 정책
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FailSafePolicy {
    /// 마스킹 없이 통과 (위험)
    PassThrough,
    /// 마지막 알려진 위치 마스킹 (권장)
    MaskLastKnown,
    /// 의심 영역 마스킹
    MaskSuspicious { min_confidence: f32 },
    /// 전체 프레임 블러
    BlurEntireFrame,
}
