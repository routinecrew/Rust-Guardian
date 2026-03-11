// ============================================================
// Rust-Guardian Mock 구현 — 독립 개발용
// ============================================================
// 각 크레이트가 다른 크레이트 없이 독립적으로 빌드/테스트할 수 있도록
// 공유 trait의 mock 구현을 제공한다.
// ============================================================

use crate::contracts::*;
use async_trait::async_trait;
use std::sync::Arc;

// ============================================================
// MockInferenceBackend — Detector 없이 테스트
// ============================================================

/// 고정된 탐지 결과를 반환하는 mock 추론 백엔드.
pub struct MockInferenceBackend {
    pub detections: Vec<Detection>,
}

impl MockInferenceBackend {
    /// 빈 탐지 결과를 반환하는 mock
    pub fn empty() -> Self {
        Self { detections: vec![] }
    }

    /// 지정된 탐지 결과를 반환하는 mock
    pub fn with_detections(detections: Vec<Detection>) -> Self {
        Self { detections }
    }

    /// 얼굴 1개를 탐지하는 mock
    pub fn face_detected() -> Self {
        Self {
            detections: vec![Detection {
                label: "face".to_string(),
                confidence: 0.95,
                bbox: BoundingBox {
                    x: 0.3,
                    y: 0.2,
                    width: 0.1,
                    height: 0.15,
                },
                track_id: Some(1),
            }],
        }
    }
}

#[async_trait]
impl InferenceBackend for MockInferenceBackend {
    async fn infer(&self, _frame: &RgbFrame) -> anyhow::Result<Vec<Detection>> {
        Ok(self.detections.clone())
    }

    fn name(&self) -> &str {
        "mock"
    }
}

// ============================================================
// MockMasker — Masker 없이 테스트
// ============================================================

/// 입력 프레임을 그대로 반환하는 mock 마스커.
pub struct MockMasker;

impl Masker for MockMasker {
    fn apply(
        &self,
        frame: &RgbFrame,
        _detections: &[Detection],
        _level: AnonymizationLevel,
    ) -> anyhow::Result<RgbFrame> {
        Ok(frame.clone())
    }
}

// ============================================================
// MockSigner — Signer 없이 테스트
// ============================================================

/// 더미 서명을 반환하는 mock 서명자.
pub struct MockSigner;

#[async_trait]
impl Signer for MockSigner {
    async fn sign(
        &self,
        _masked_data: &[u8],
        _detections: &[Detection],
        anonymization_level: AnonymizationLevel,
    ) -> anyhow::Result<PrivacySignature> {
        Ok(PrivacySignature {
            device_id: "mock-device".to_string(),
            timestamp: 0,
            sequence: 0,
            prev_frame_hash: [0u8; 32],
            nonce: [0u8; 16],
            frame_hash: [0u8; 32],
            detection_summary: "mock".to_string(),
            model_version: "mock-v1".to_string(),
            policy_hash: [0u8; 32],
            anonymization_level: anonymization_level as u8,
            chain_status: ChainStatus::Normal,
            signature_bytes: vec![0u8; 64],
        })
    }
}

// ============================================================
// MockCaptureSource — Capture 없이 테스트
// ============================================================

/// 테스트 프레임을 생성하는 mock 입력 소스.
pub struct MockCaptureSource {
    frame_count: u32,
    max_frames: u32,
}

impl MockCaptureSource {
    pub fn new(max_frames: u32) -> Self {
        Self {
            frame_count: 0,
            max_frames,
        }
    }
}

#[async_trait]
impl CaptureSource for MockCaptureSource {
    async fn next_frame(&mut self) -> anyhow::Result<Option<SecureBytes>> {
        if self.frame_count >= self.max_frames {
            return Ok(None);
        }
        self.frame_count += 1;
        // 320x240 RGB 테스트 프레임 (단색)
        let size = 320 * 240 * 3;
        let data = vec![128u8; size];
        Ok(Some(SecureBytes::from_raw(data)))
    }

    fn name(&self) -> &str {
        "mock-capture"
    }

    async fn stop(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

// ============================================================
// MockOutputSink — Output 없이 테스트
// ============================================================

/// 아무것도 하지 않는 mock 출력.
pub struct MockOutputSink;

#[async_trait]
impl OutputSink for MockOutputSink {
    async fn write(&self, _frame: &ProcessedFrame) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "mock-output"
    }

    async fn stop(&self) -> anyhow::Result<()> {
        Ok(())
    }
}

// ============================================================
// 테스트 유틸리티
// ============================================================

/// 테스트용 RGB 프레임을 생성한다.
pub fn test_rgb_frame(width: u32, height: u32) -> RgbFrame {
    RgbFrame {
        width,
        height,
        data: vec![128u8; (width * height * 3) as usize],
    }
}

/// 테스트용 GuardianConfig를 생성한다.
pub fn test_config() -> GuardianConfig {
    GuardianConfig {
        capture: CaptureConfig {
            source: CaptureSourceType::File,
            path: "test.jpg".to_string(),
            width: Some(320),
            height: Some(240),
            fps: Some(5),
        },
        detector: DetectorConfig {
            model_path: "models/yunet.onnx".to_string(),
            backend: "ort".to_string(),
            input_width: 320,
            input_height: 320,
            confidence_threshold: 0.5,
            nms_threshold: 0.4,
            target_labels: vec!["face".to_string()],
            quantization: None,
        },
        masker: MaskerConfig {
            level: 1,
            legal_review_required: None,
        },
        signer: None,
        audio: None,
        output: OutputConfig {
            sinks: vec![OutputSinkConfig {
                sink_type: "file".to_string(),
                path: Some("output/".to_string()),
                url: None,
            }],
        },
        api: None,
        thermal: None,
        ota: None,
        audit: None,
        agent: None,
    }
}
