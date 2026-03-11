# Agent C: guardian-detector + guardian-masker 개발 스킬

## 너의 역할
Rust-Guardian 프로젝트의 **프라이버시 파이프라인 핵심**을 만든다.
얼굴/번호판을 탐지하고, 비식별화 필터를 적용하는 것이 이 프로젝트의 존재 이유다.
**Recall ≥ 0.92는 절대 기준** — 얼굴 1명을 놓치면 사고다.

## 반드시 지킬 것
- `contracts/shared_types.rs`의 `InferenceBackend`, `Masker` trait을 구현할 것
- NMS(Non-Maximum Suppression) 후처리 필수
- FailSafePolicy 구현: 탐지 실패 시 전체 블러 모드로 전환
- AccuracyMonitor로 Recall 자가 검증

## 구현 대상

### guardian-detector

#### 1. InferenceBackend 구현체
```rust
// ORT (ONNX Runtime) — 기본 CPU/CUDA 백엔드
pub struct OrtBackend { session: ort::Session }

#[async_trait]
impl InferenceBackend for OrtBackend {
    async fn infer(&self, frame: &RgbFrame) -> Result<Vec<Detection>>;
    fn name(&self) -> &str { "ort" }
}

// NCNN — RPi 최적화
pub struct NcnnBackend { /* ... */ }

// TFLite — Edge TPU
pub struct TfLiteBackend { /* ... */ }
```

#### 2. 하드웨어 자동 감지 (hardware.rs)
```rust
pub fn detect_hardware() -> Hardware {
    // /proc/device-tree/model, lspci, nvidia-smi 등으로 판별
}

pub fn select_backend(config: &DetectorConfig) -> Box<dyn InferenceBackend> {
    match detect_hardware() {
        Hardware::X86Cpu => Box::new(OrtBackend::new_cpu(config)),
        Hardware::X86WithCuda => Box::new(OrtBackend::new_cuda(config)),
        Hardware::Rpi4 | Hardware::Rpi5 => Box::new(NcnnBackend::new(config)),
        // ...
    }
}
```

#### 3. DetectionTracker (tracker.rs)
- 연속 프레임 탐지 안정화 (IoU 기반 매칭)
- confirmation_frames: N프레임 연속 감지 시 확정
- max_age: M프레임 미감지 시 트랙 삭제
- FailSafePolicy 적용

#### 4. AccuracyMonitor (accuracy_monitor.rs)
- 벤치마크 이미지셋으로 정기 자가 검증
- Recall 미달 시 전체 블러 모드 전환 + 관리자 알림
- check_interval: 기본 24시간

#### 5. NMS (nms.rs)
- Non-Maximum Suppression 구현
- 불변량: NMS 후 모든 박스 쌍의 IoU < threshold

#### 6. 전처리 (preprocess.rs)
- RGB 리사이즈 (320×320 / 640×640)
- f32 텐서 변환
- 정규화 (0~255 → 0~1)

### guardian-masker

#### 1. Masker trait 구현
```rust
pub struct GuardianMasker;

impl Masker for GuardianMasker {
    fn apply(
        &self,
        frame: &RgbFrame,
        detections: &[Detection],
        level: AnonymizationLevel,
    ) -> Result<RgbFrame> {
        match level {
            Level1GaussianBlur => self.gaussian_blur(frame, detections),
            Level2StrongMosaic => self.mosaic(frame, detections),
            Level3SolidBox     => self.solid_box(frame, detections),
            Level4Synthetic    => self.synthetic_replace(frame, detections),
            Level5Removal      => self.remove(frame, detections),
        }
    }
}
```

#### 2. 각 등급 구현
- Level 1: Gaussian Blur (sigma 자동 조절)
- Level 2: Mosaic (블록 크기 조절)
- Level 3: Solid Color Box
- Level 4: Synthetic Replacement (Phase 3 이후 — stub)
- Level 5: Inpainting / Solid Fill

## 의존성
`image` 크레이트로 이미지 처리. ORT는 `ort = "2"` 사용.
Phase 0에서는 ORT만 구현, NCNN/TFLite는 stub.

## guardian-core 없이 먼저 개발하는 방법
contracts의 타입과 mock을 크레이트 내부에 복사하여 사용:
```rust
// MockInferenceBackend으로 Masker 독립 테스트
let detector = MockInferenceBackend::face_detected();
let masker = GuardianMasker;
let frame = test_rgb_frame(320, 240);
let detections = detector.infer(&frame).await?;
let masked = masker.apply(&frame, &detections, AnonymizationLevel::Level1GaussianBlur)?;
```

## 테스트 시나리오
1. OrtBackend: 테스트 이미지 → YuNet → 얼굴 Detection 확인
2. NMS: 겹치는 10개 Detection → NMS → 3개로 줄어듦 확인
3. DetectionTracker: 3프레임 연속 → track_id 할당
4. AccuracyMonitor: Recall < 0.92 → fail-safe 트리거
5. Masker Level 1: 얼굴 영역 블러 확인 (바깥은 원본 유지)
6. Masker Level 3: 얼굴 영역 단색 박스
7. FailSafe: Detection 에러 → 전체 프레임 블러

## 완료 기준
- `cargo test -p guardian-detector` 전부 통과
- `cargo test -p guardian-masker` 전부 통과
- Phase 0: 이미지 1장 → YuNet 얼굴 탐지 → Gaussian Blur 적용 → 출력
- 성능: CPU ORT 기준 프레임당 < 100ms (320×320)
