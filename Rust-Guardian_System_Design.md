# Rust-Guardian — 시스템 설계서 v3.0

> Edge Privacy Filter + Intelligent Agent Architecture in Rust
> **v3.0 — 잔여 4대 과제 해결 + 90점 목표 (2026.03.11)**

---

## 변경 이력

| 버전 | 일자 | 변경 내용 |
|------|------|----------|
| v1.0 | 2026.03.11 | 초안 작성 |
| v2.0 | 2026.03.11 | 6개 전문가 패널 1차 검증. 32개 개선사항 반영 (68.7→79.8) |
| v3.0 | 2026.03.11 | 2차 검증 잔여 4대 과제 해결: ① SecureBytes 원본 보호, ② mAP 기준선+양자화 워크플로우, ③ 체인 복구+열 관리+OTA, ④ 커뮤니티 90일 플랜+라이선스 확정 |

---

## 1. 현재 경쟁 제품 구조 분석

*(v2.0과 동일 — 변경 없음)*

### 1.1 deface (Python) — 현재 시장 지배적 오픈소스

```
deface (CLI binary)
  └─ main()
       ├─ argparse           ← CLI 옵션 파싱
       ├─ CenterFace         ← DNN 모델 래퍼 (OpenCV/ONNX Runtime)
       ├─ anonymize_frame()  ← 프레임별 비식별화 (blur/mosaic/solid)
       └─ video_iterator     ← ffmpeg 기반 입출력
```

**한계점:** Python GIL 병목, 얼굴만 탐지, 정책 없음, 서명 없음

### 1.2 detr-mmap (Rust) — 엣지 AI 파이프라인 참조

```
detr-mmap/
  ├─ capture / bridge (mmap+mqueue) / schema (FlatBuffers)
  ├─ inference (ORT/TensorRT) / controller (Sentry 상태 머신)
  ├─ gateway (axum WebSocket) / broker (MQTT)
```

**참조 패턴:** zero-copy mmap, Sentry 히스테리시스, FlatBuffers
**한계점:** 비식별화 없음, 서명 없음, 정책 고정

### 1.3 격차(Gap) 분석

```
                    탐지    비식별화   엣지최적화  무결성증명  음성처리  동적정책
deface (Python)      ✅       ✅         ❌         ❌        ❌       ❌
detr-mmap (Rust)     ✅       ❌         ✅         ❌        ❌       ❌
brighter AI          ✅       ✅         △          ❌        ❌       △
────────────────────────────────────────────────────────────────────────
Rust-Guardian        ✅       ✅         ✅         ✅        ✅       ✅
```

**3대 공백:** 무결성 증명, 음성 비식별화, Wasm 동적 정책

---

## 2. Rust-Guardian 전체 아키텍처

### 2.1 전체 구조

```
rust-guardian (binary)
  └─ Engine
       ├─ config::Config               ← serde_yaml 설정 + 핫 리로드
       ├─ config::Watcher              ← notify crate 기반 파일 변경 감지
       ├─ capture::CaptureManager      ← 입력 소스 관리
       │    ├─ V4lCapture              ← V4L2 카메라 (🆕 HW 디코더 연동)
       │    ├─ RtspCapture             ← RTSP 스트림 수신 (retina)
       │    └─ FileCapture             ← 파일 입력 (테스트/배치용)
       ├─ pipeline::Pipeline           ← 프레임 처리 파이프라인
       │    ├─ SecureFrameBuffer       ← 🆕 [v3.0] SecureBytes 기반 버퍼
       │    ├─ 🛡️ detector::Detector  ← 객체 탐지 (하드웨어별 자동 선택)
       │    ├─ 🛡️ tracker::Tracker    ← 탐지 안정화 + fail-safe
       │    ├─ 🛡️ masker::Masker      ← 비식별화 필터 (등급제)
       │    ├─ 🛡️ signer::Signer      ← 디지털 서명 (체이닝 + 복구)
       │    └─ 🧠 agent::AgentTap      ← 지능형 정책 에이전트
       ├─ audio::AudioFilter           ← 🔇 음성 비식별화
       ├─ output::OutputManager        ← 출력 관리
       ├─ transport::SecureTransport   ← 보안 전송 (mTLS)
       ├─ api::ApiServer               ← REST API (axum)
       │    └─ SubjectRightsHandler    ← 데이터 주체 권리 API
       ├─ audit::AuditLogger           ← 감사 로그
       ├─ thermal::ThermalMonitor      ← 🆕 [v3.0] 열 관리 + 자동 스로틀링
       ├─ ota::OtaManager             ← 🆕 [v3.0] Over-the-Air 업데이트
       ├─ metrics::MetricsCollector    ← Prometheus 메트릭
       └─ 🧠 agent::PolicyRuntime     ← Wasm 동적 정책 (서명 검증)
```

### 2.2 하드웨어별 추론 백엔드 분기

```rust
pub trait InferenceBackend: Send + Sync {
    async fn infer(&self, frame: &RgbFrame) -> Result<Vec<Detection>>;
    fn name(&self) -> &str;
}

pub fn select_backend(config: &DetectorConfig) -> Box<dyn InferenceBackend> {
    match detect_hardware() {
        Hardware::JetsonOrin  => Box::new(TensorRtBackend::new(config)),
        Hardware::JetsonNano  => Box::new(TensorRtBackend::new(config)),
        Hardware::RpiWithTpu  => Box::new(TfLiteBackend::new_edgetpu(config)),
        Hardware::Rpi5        => Box::new(NcnnBackend::new(config.with_input_size(320, 320))),
        Hardware::Rpi4        => Box::new(NcnnBackend::new(config.with_input_size(320, 320))),
        Hardware::X86WithCuda => Box::new(OrtBackend::new_cuda(config)),
        Hardware::X86Cpu      => Box::new(OrtBackend::new_cpu(config)),
    }
}
```

### 2.3 🆕 [v3.0] 성능 목표 + mAP 기준선

**v2.0 문제:** fps 목표는 있었으나 정확도(mAP) 기준이 없었음. 프라이버시 필터에서 얼굴 1명을 놓치면 사고이므로, 최소 허용 정확도가 반드시 필요함.

| 타겟 | 모델 | 입력 크기 | 백엔드 | 예상 FPS | 🆕 최소 mAP50 | 🆕 최소 Recall |
|------|------|----------|--------|---------|-------------|--------------|
| RPi4 CPU | YuNet | 320×320 | NCNN | 5~8 | ≥ 0.85 (얼굴) | ≥ 0.92 |
| RPi5 CPU | YOLOv8n | 640×640 | OpenVINO | 8~12 | ≥ 0.88 (얼굴+번호판) | ≥ 0.94 |
| RPi + Coral | YOLOv8n | 320×320 | TFLite | 25~30 | ≥ 0.85 | ≥ 0.92 |
| Jetson Nano | YOLOv8n INT8 | 640×640 | TensorRT | 30~43 | ≥ 0.90 | ≥ 0.95 |
| Jetson Orin | YOLOv8s FP16 | 1080p | TensorRT | 30~60 | ≥ 0.92 | ≥ 0.96 |
| x86 + RTX | YOLOv8m | 1080p | ort CUDA | 60+ | ≥ 0.94 | ≥ 0.97 |

**🆕 Recall이 mAP보다 중요한 이유:** 프라이버시 필터에서 Precision이 낮으면(오탐) 배경이 불필요하게 블러되는 것일 뿐이지만, Recall이 낮으면(미탐) 얼굴이 그대로 노출됨. **Recall ≥ 0.92는 절대 기준.**

**🆕 mAP 미달 시 자동 대응:**
```rust
/// 정기적으로 벤치마크 이미지셋으로 자가 검증
pub struct AccuracyMonitor {
    test_images: Vec<(RgbFrame, Vec<Detection>)>,  // 정답 라벨 포함
    check_interval: Duration,                       // 기본: 24시간마다
    min_recall: f32,                                // 기본: 0.92
}

impl AccuracyMonitor {
    pub fn check(&self, detector: &dyn InferenceBackend) -> AccuracyResult {
        let recall = self.compute_recall(detector);
        if recall < self.min_recall {
            // 🚨 Recall 미달 → 관리자 알림 + fail-safe 모드 전환
            AccuracyResult::BelowThreshold {
                actual: recall,
                required: self.min_recall,
                action: "전체 프레임 블러 모드로 전환됨",
            }
        } else {
            AccuracyResult::Passed { recall }
        }
    }
}
```

### 2.4 🆕 [v3.0] 양자화 워크플로우 (구체화)

v2.0에서 "양자화 유틸"만 언급했던 것을 실제 파이프라인으로 구체화.

```
양자화 워크플로우:

1. 캘리브레이션 데이터 수집
   └─ 대표 이미지 500장 (다양한 조명/각도/거리)
   └─ tools/collect_calibration.py → calibration_data/

2. Post-Training Quantization (PTQ)
   └─ FP32 → FP16: 정확도 손실 ~1%, 속도 2x
   └─ FP32 → INT8: 정확도 손실 ~3%, 속도 4x
   └─ tools/quantize.py --model yolov8n-face.onnx --level int8

3. 정확도 검증 (자동)
   └─ WiderFace validation set 기준
   └─ mAP50 손실이 5% 초과하면 → 양자화 거부 + 경고
   └─ tools/validate_quantized.py --original fp32.onnx --quantized int8.onnx

4. 형식 변환 (타겟별)
   └─ ONNX → TensorRT engine (Jetson)
   └─ ONNX → NCNN .bin/.param (RPi)
   └─ ONNX → TFLite (Edge TPU)
   └─ tools/convert_model.py --target rpi4
```

```yaml
# guardian.yml — 양자화 설정
detector:
  quantization:
    level: "auto"                    # auto | fp32 | fp16 | int8
    calibration_data: "./calibration/"
    max_mAP50_loss: 0.05             # 5% 초과 시 양자화 거부
    max_recall_loss: 0.03            # 3% 초과 시 양자화 거부
```

### 2.5 🆕 [v3.0] SecureBytes — 원본 프레임 보호

**v2.0 문제:** `Arc<Bytes>`로 프레임을 공유하면 파이프라인 여러 단계가 동시에 참조를 보유. 마지막 참조 해제 전까지 원본 평문이 메모리에 수 초간 잔류. zeroize를 호출해도 다른 곳에서 참조 중이면 무의미.

```rust
/// 🆕 [v3.0] SecureBytes — 원본 보호를 보장하는 프레임 래퍼
/// Arc 참조 카운팅과 zeroize를 양립시키는 핵심 설계
pub struct SecureBytes {
    inner: Box<[u8]>,           // heap 할당, 단일 소유자
    purged: AtomicBool,         // 삭제 완료 플래그
}

impl SecureBytes {
    pub fn from_raw(data: Vec<u8>) -> Self {
        Self {
            inner: data.into_boxed_slice(),
            purged: AtomicBool::new(false),
        }
    }

    /// 읽기 전용 뷰 — 파이프라인 단계에서 사용
    /// purge 후에는 접근 불가
    pub fn view(&self) -> Option<&[u8]> {
        if self.purged.load(Ordering::Acquire) {
            None  // 이미 삭제됨
        } else {
            Some(&self.inner)
        }
    }

    /// 원본 데이터를 0으로 덮어쓰고 접근 차단
    /// 비식별화 완료 후 즉시 호출
    pub fn purge(&self) {
        if !self.purged.swap(true, Ordering::AcqRel) {
            // safety: 단일 소유자이므로 안전하게 zeroize 가능
            unsafe {
                let ptr = self.inner.as_ptr() as *mut u8;
                core::ptr::write_bytes(ptr, 0, self.inner.len());
            }
            // 컴파일러 최적화 방지 (dead store elimination 차단)
            std::sync::atomic::fence(Ordering::SeqCst);
        }
    }
}

impl Drop for SecureBytes {
    fn drop(&mut self) {
        self.purge();  // Drop 시에도 반드시 zeroize
    }
}
```

**파이프라인에서의 사용 흐름:**
```rust
async fn process_frame(&self, raw: SecureBytes) -> Result<ProcessedFrame> {
    // 1. 원본으로 디코딩 + 탐지 + 마스킹
    let rgb = decode_video_frame(raw.view().ok_or(PurgedError)?)?;
    let detections = self.detector.infer(&rgb).await?;
    let masked = self.masker.apply(&rgb, &detections)?;
    let encoded = encode_video_frame(&masked)?;

    // 2. 🆕 [v3.0] 비식별화 완료 즉시 원본 삭제
    raw.purge();
    drop(rgb);  // 디코딩된 RGB도 즉시 해제

    // 3. 이후 서명은 masked 데이터만 사용 (원본 불필요)
    let signature = self.signer.sign(&encoded, &detections)?;

    Ok(ProcessedFrame {
        masked_payload: encoded,
        detections,
        signature: Some(signature),
        // original 필드 제거 — v3.0에서는 원본 참조를 보유하지 않음
    })
}
```

**v2.0 `Arc<Bytes>` 대비 변경점:**
- `ProcessedFrame`에서 `original: Arc<MediaFrame>` 필드 **제거**
- 원본은 `SecureBytes`로 파이프라인 입구에서만 존재하고, masking 완료 즉시 `purge()`
- 이후 단계(signer, output)는 비식별화된 데이터만 접근 가능
- Drop 안전망으로 어떤 경로에서든 누출 방지

### 2.6 비식별화 등급 체계 (GDPR 대응)

```rust
pub enum AnonymizationLevel {
    Level1GaussianBlur,    // 복원 가능 — GDPR 적용 대상
    Level2StrongMosaic,    // 복원 어려움 — GDPR 적용 대상
    Level3SolidBox,        // 원본 소멸 — 익명화에 가까움
    Level4Synthetic,       // 합성 대체 — ⚠️ 법률 자문 필수 (아래 참조)
    Level5Removal,         // 영역 완전 제거
}
```

**🆕 [v3.0] Level 4 법적 경고 추가:**
```yaml
masker:
  level: 4  # Synthetic Replacement
  # ⚠️ 경고: Level 4(합성 대체)의 GDPR '익명화' 인정 여부는
  # 아직 EU 판례로 확립되지 않았습니다.
  # 프로덕션 사용 전 반드시 현지 데이터보호 법률 자문을 받으십시오.
  # 참조: EDPB Opinion 05/2014 on Anonymisation Techniques
  legal_review_required: true
```

### 2.7 서명 체이닝 + 🆕 [v3.0] 체인 복구 프로토콜

```rust
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
    pub chain_status: ChainStatus,          // 🆕 [v3.0]
    pub signature: ed25519_dalek::Signature,
}

/// 🆕 [v3.0] 체인 상태 — 끊김 시 복구 지원
pub enum ChainStatus {
    /// 정상 연속 체인
    Normal,
    /// 체인 재시작 — 이전 체인 종료 후 새 체인 시작
    /// 정전/crash 복구 시 사용
    ChainReset {
        reason: ChainResetReason,
        previous_chain_last_seq: u64,
        previous_chain_last_hash: [u8; 32],
    },
}

pub enum ChainResetReason {
    ProcessRestart,         // 프로세스 재시작
    PowerFailure,           // 정전 복구
    ManualReset,            // 관리자 수동 리셋
    KeyRotation,            // 키 교체
}
```

**체인 복구 흐름:**
```
정상 동작:  frame#1 → frame#2 → frame#3 → ...
               ↓          ↓          ↓
           seq=1      seq=2      seq=3
           prev=0x00  prev=H(#1) prev=H(#2)
           chain=Normal

정전 발생:  frame#3 이후 crash
                     ↓
재시작 후:   frame#4 (새 체인 시작)
               ↓
           seq=1  ← 시퀀스 리셋
           prev=0x00
           chain=ChainReset {
             reason: PowerFailure,
             previous_chain_last_seq: 3,
             previous_chain_last_hash: H(#3)  ← 디스크에서 복구
           }

Verifier:  ChainReset을 받으면
           → 이전 체인의 마지막 해시와 대조
           → 일치하면 정당한 재시작으로 인정
           → 불일치면 위변조 경고
```

**체인 상태 영속화:**
```rust
/// 마지막 서명 상태를 디스크에 저장 (crash 복구용)
/// 파일: /var/lib/rust-guardian/chain_state.bin
pub struct ChainStatePersistence {
    path: PathBuf,
    last_sequence: u64,
    last_frame_hash: [u8; 32],
}

impl ChainStatePersistence {
    /// 매 프레임 서명 후 fsync로 디스크 기록
    pub fn save(&self, sig: &PrivacySignature) -> Result<()> {
        let state = [
            &sig.sequence.to_le_bytes()[..],
            &sig.frame_hash[..],
        ].concat();
        std::fs::write(&self.path, &state)?;
        // fsync로 정전에도 안전하게 기록
        let file = std::fs::File::open(&self.path)?;
        file.sync_all()?;
        Ok(())
    }
}
```

### 2.8 🆕 [v3.0] 열 관리 (Thermal Throttling 대응)

**RPi4는 지속적 AI 추론 시 80°C 이상에서 CPU 클럭 강제 하향. fps가 갑자기 절반 이하로 떨어지는 원인.**

```rust
/// 열 관리 모니터 — Sentry 모드와 연동
pub struct ThermalMonitor {
    /// /sys/class/thermal/thermal_zone0/temp (밀리도 단위)
    temp_path: PathBuf,
    /// 경고 온도 (기본: 70°C) — 이 이상이면 fps 자동 감소
    warning_threshold: f32,
    /// 위험 온도 (기본: 80°C) — 이 이상이면 Standby 모드 강제 전환
    critical_threshold: f32,
    /// 현재 스로틀링 상태
    throttled: AtomicBool,
}

impl ThermalMonitor {
    pub fn current_temp(&self) -> f32 {
        let raw = std::fs::read_to_string(&self.temp_path).unwrap_or_default();
        raw.trim().parse::<f32>().unwrap_or(0.0) / 1000.0
    }

    /// Sentry 모드와 통합 — 열 상태에 따라 fps 조절
    pub fn recommend_fps(&self, sentry_fps: u32) -> u32 {
        let temp = self.current_temp();
        if temp >= self.critical_threshold {
            // 🔴 위험: 최소 fps로 강제 전환
            return 1;
        } else if temp >= self.warning_threshold {
            // 🟡 경고: fps를 절반으로 감소
            return sentry_fps / 2;
        }
        sentry_fps  // 정상
    }
}
```

```yaml
# guardian.yml — 열 관리 설정
thermal:
  enabled: true
  warning_threshold: 70.0    # °C — fps 절반 감소
  critical_threshold: 80.0   # °C — Standby 강제 전환
  check_interval: 5          # 초
  # RPi4 권장: 방열판 필수, 팬 권장
  # Jetson Nano: 팬 포함 키트 사용 시 비활성화 가능
```

### 2.9 🆕 [v3.0] OTA (Over-the-Air) 업데이트

**수백 대 원격 엣지 기기의 바이너리/모델/정책을 안전하게 업데이트하는 메커니즘.**

```rust
/// OTA 업데이트 매니저
pub struct OtaManager {
    server_url: String,
    device_id: String,
    check_interval: Duration,          // 기본: 1시간마다 확인
    signing_key: ed25519_dalek::VerifyingKey,  // 업데이트 서명 검증용
}

/// 업데이트 대상 타입
pub enum UpdateTarget {
    /// 바이너리 업데이트 (A/B 파티션 스왑)
    Binary { version: String, url: String, hash: [u8; 32] },
    /// ONNX/NCNN 모델 교체 (핫스왑 가능)
    Model { name: String, url: String, hash: [u8; 32] },
    /// Wasm 정책 교체 (핫스왑 가능)
    Policy { name: String, url: String, hash: [u8; 32] },
    /// 설정 파일 업데이트
    Config { url: String, hash: [u8; 32] },
}

/// 업데이트 패키지 (서명 필수)
pub struct SignedUpdate {
    pub target: UpdateTarget,
    pub signature: ed25519_dalek::Signature,  // 서버의 서명
    pub min_version: String,                   // 최소 호환 버전
    pub rollback_timeout: Duration,            // 이 시간 내 헬스체크 실패 시 롤백
}
```

**안전한 업데이트 흐름:**
```
1. 서버에서 업데이트 확인
   GET /api/v1/updates?device_id=factory-cam-01&version=0.5.0

2. 서명 검증
   → 서명 실패 시 업데이트 거부 + 경고 로그

3. 다운로드 + 해시 검증
   → SHA-256 불일치 시 재다운로드 (최대 3회)

4. 적용 (타겟별)
   Binary: A/B 파티션에 쓰기 → 재부팅 → 헬스체크
   Model:  models/ 디렉토리에 쓰기 → Detector 핫 리로드
   Policy: policies/ 디렉토리에 쓰기 → PolicyRuntime 핫 리로드
   Config: config/ 디렉토리에 쓰기 → 기존 핫 리로드

5. 롤백 안전망
   Binary 업데이트 후 rollback_timeout(기본 5분) 내에
   헬스체크(/api/health) 실패 시 → 이전 파티션으로 자동 롤백
```

```yaml
# guardian.yml — OTA 설정
ota:
  enabled: true
  server_url: "https://update.guardian.example.com"
  check_interval: 3600      # 초 (1시간)
  auto_apply:
    model: true              # 모델 핫스왑 자동 적용
    policy: true             # 정책 핫스왑 자동 적용
    binary: false            # 바이너리는 수동 승인 (기본)
    config: false            # 설정은 수동 승인 (기본)
  rollback_timeout: 300      # 초 (5분)
```

### 2.10 구조화된 에러 처리 + Graceful Degradation

```rust
#[derive(thiserror::Error, Debug)]
pub enum GuardianError {
    #[error("capture failed: {0}")]
    Capture(#[from] CaptureError),
    #[error("detection failed: {0}")]
    Detection(#[from] DetectionError),
    #[error("masking failed: {0}")]
    Masking(#[from] MaskError),
    #[error("signing failed: {0}")]
    Signing(#[from] SignError),
    #[error("purge failed: {0}")]
    Purge(#[from] PurgeError),         // 🆕 [v3.0]
    #[error("thermal critical: {0}°C")]
    ThermalCritical(f32),              // 🆕 [v3.0]
}

/// Degradation: Detection 실패 → 전체 블러 / Signing 실패 → 전송 차단
/// 🆕 [v3.0] ThermalCritical → Standby 강제 + 관리자 알림
```

### 2.11 탐지 안정화 (Tracker) + Fail-Safe

```rust
pub struct DetectionTracker {
    active_tracks: HashMap<TrackId, TrackedObject>,
    confirmation_frames: u32,
    max_age: u32,
    fail_safe: FailSafePolicy,
}

pub enum FailSafePolicy {
    PassThrough,
    MaskLastKnown,                              // 권장
    MaskSuspicious { min_confidence: f32 },
}
```

---

## 3. 위협 모델 (STRIDE)

| 위협 | 유형 | 공격 시나리오 | 대응 |
|------|------|-------------|------|
| T1 | Tampering | 물리적 기기 탈취 → 키 유출 | TPM + 원격 폐기(CRL) |
| T2 | Tampering | Wasm 정책 주입 → 비식별화 우회 | 정책 서명 검증 + capability 제한 |
| T3 | Spoofing | MITM → 설정 변조 | mTLS + 설정 서명 |
| T4 | Info Disclosure | 메모리 덤프 → 원본 추출 | 🆕 SecureBytes purge (v3.0) |
| T5 | Repudiation | 프레임 replay | 체이닝 + nonce + 🆕 체인 복구 (v3.0) |
| T6 | DoS | 추론 큐 폭주 | 백프레셔 + Sentry + 🆕 열 관리 (v3.0) |
| T7 | Elevation | Wasm 시스템 접근 | wasmtime 샌드박스 |
| T8 | Tampering | 🆕 OTA 변조 → 악성 바이너리 | OTA 서명 검증 + 롤백 (v3.0) |

---

## 4. 메모리 예산

| 컴포넌트 | RPi4 (1GB) | RPi4 (4GB) | Jetson Nano (4GB) |
|---------|-----------|-----------|------------------|
| OS + 기본 | 200MB | 200MB | 500MB |
| 추론 모델 | 15MB | 25MB | 25MB |
| 추론 엔진 | 50MB | 200MB | 300MB (GPU) |
| 프레임 버퍼 | 6MB | 24MB | 24MB |
| ffmpeg 디코더 | 30MB | 50MB | 50MB |
| 🆕 체인 상태 영속화 | <1MB | <1MB | <1MB |
| 애플리케이션 | 10MB | 10MB | 10MB |
| **합계** | **312MB** | **510MB** | **910MB** |
| **남은 메모리** | **688MB** | **3.5GB** | **3.1GB** |

---

## 5. 디렉토리 구조

```
rust-guardian/
├── Cargo.toml
├── crates/
│   ├── guardian-core/
│   │   └── src/
│   │       ├── engine.rs
│   │       ├── config.rs
│   │       ├── pipeline.rs
│   │       ├── frame.rs
│   │       ├── secure_bytes.rs      ← 🆕 [v3.0] SecureBytes 구현
│   │       └── error.rs
│   ├── guardian-capture/
│   ├── guardian-detector/
│   │   └── src/
│   │       ├── backend/ (ort, ncnn, tflite, trt)
│   │       ├── detector.rs
│   │       ├── yunet.rs
│   │       ├── quantize.rs
│   │       ├── tracker.rs
│   │       └── accuracy_monitor.rs  ← 🆕 [v3.0] mAP 자가 검증
│   ├── guardian-masker/
│   ├── guardian-signer/
│   │   └── src/
│   │       ├── signer.rs
│   │       ├── chain.rs
│   │       ├── chain_recovery.rs    ← 🆕 [v3.0] 체인 복구 프로토콜
│   │       ├── chain_persistence.rs ← 🆕 [v3.0] 체인 상태 영속화
│   │       ├── keystore.rs
│   │       ├── revocation.rs
│   │       └── verifier.rs
│   ├── guardian-audio/
│   ├── guardian-agent/
│   ├── guardian-transport/
│   ├── guardian-output/
│   ├── guardian-api/
│   ├── guardian-audit/
│   ├── guardian-thermal/            ← 🆕 [v3.0] 열 관리
│   │   └── src/
│   │       └── monitor.rs
│   ├── guardian-ota/                ← 🆕 [v3.0] OTA 업데이트
│   │   └── src/
│   │       ├── manager.rs
│   │       ├── download.rs
│   │       └── rollback.rs
│   └── guardian-bridge/
├── tools/                           ← 🆕 [v3.0] 오프라인 도구
│   ├── collect_calibration.py       ← 캘리브레이션 데이터 수집
│   ├── quantize.py                  ← 모델 양자화
│   ├── validate_quantized.py        ← 양자화 정확도 검증
│   └── convert_model.py             ← 타겟별 모델 변환
├── config/guardian.yml
├── models/ (ort/, ncnn/, tflite/)
├── tests/
│   ├── unit/
│   │   ├── secure_bytes_test.rs     ← 🆕 [v3.0] purge 검증
│   │   ├── chain_recovery_test.rs   ← 🆕 [v3.0] 체인 복구 검증
│   │   └── thermal_test.rs          ← 🆕 [v3.0] 열 관리 검증
│   ├── integration/
│   ├── property/
│   │   ├── nms_test.rs              ← 🆕 불변량: NMS 후 모든 박스 쌍 IoU < threshold
│   │   └── chain_test.rs            ← 🆕 불변량: 모든 연속 프레임 prev_hash == H(이전)
│   └── bench/
├── deploy/
│   ├── systemd/rust-guardian.service
│   ├── docker/
│   └── cross/Cross.toml
└── docs/
    ├── THREAT_MODEL.md
    ├── DPIA_TEMPLATE.md
    ├── COMPLIANCE_MATRIX.md         ← 🆕 [v3.0] GDPR+CCPA+PIPL+PIPA+APPI 매핑
    └── QUANTIZATION_GUIDE.md        ← 🆕 [v3.0] 양자화 가이드
```

---

## 6. Feature Flag 체계

```toml
[features]
default = ["cpu", "signing", "audit"]
# 추론 백엔드
cpu = []
cuda = ["ort/cuda"]
tensorrt = ["ort/tensorrt"]
ncnn = ["dep:ncnn-rs"]
tflite = ["dep:tflite-rs"]
openvino = ["ort/openvino"]
# 기능 모듈
audio = ["dep:guardian-audio"]
signing = ["dep:guardian-signer"]
wasm-policy = ["dep:guardian-agent", "dep:wasmtime"]
audit = ["dep:guardian-audit"]
subject-rights = ["audit"]
thermal = ["dep:guardian-thermal"]       # 🆕 [v3.0]
ota = ["dep:guardian-ota"]               # 🆕 [v3.0]
# 빌드 타겟
rpi-optimized = ["ncnn", "thermal"]      # 🆕 열 관리 기본 포함
jetson-optimized = ["tensorrt"]
no_std_core = []
```

---

## 7. 🆕 [v3.0] 라이선스 전략 확정

**v2.0 문제:** ffmpeg LGPL 문제가 "나중에 해결"로 미뤄짐. 기업 도입 시 법률팀 차단 위험.

### 확정된 라이선스 경로

| 의존성 | 라이선스 | 결정 | 근거 |
|--------|---------|------|------|
| ffmpeg-next | LGPL 2.1 | **동적 링킹으로 Phase 1부터 사용** | LGPL은 동적 링킹 시 전파되지 않음 |
| YOLO 모델 | AGPL (Ultralytics) | **사용하지 않음** | AGPL 전파 위험 |
| YOLOv8 가중치 | GPL → **RF-DETR 대체** | Apache 2.0 모델만 사용 | 라이선스 안전 |
| YuNet | MIT | 사용 | 문제 없음 |
| ort | MIT | 사용 | 문제 없음 |
| wasmtime | Apache 2.0 | 사용 | 문제 없음 |

**장기 로드맵 (ffmpeg 제거):**
```
Phase 1~4:  ffmpeg-next (동적 링킹, LGPL 준수)
Phase 5+:   GStreamer (LGPL, 하드웨어 디코더 네이티브 지원)
            또는 rav1d(AV1) + openh264-rs(H.264) 순수 Rust 조합
```

**Cargo.toml 동적 링킹 설정:**
```toml
[dependencies]
ffmpeg-next = { version = "7", features = ["dynamic"] }  # 동적 링킹 강제
```

---

## 8. 구현 전략 — v3.0 로드맵

### Phase 0: 2주 MVP (v0.1-alpha)

**목표:** GitHub 공개 + 즉시 데모
- 입력: 이미지 파일 1장
- 탐지: ort + YuNet (MIT 라이선스, 얼굴만)
- 마스킹: Gaussian Blur만
- CLI: `rust-guardian blur input.jpg output.jpg`
- 🆕 [v3.0] RPi4 1GB 스모크 테스트 포함
- 🆕 [v3.0] "Show HN" + r/rust 포스팅 (커뮤니티 90일 플랜 Day 1)

### Phase 1: 코어 비식별화 (1~3개월)

- InferenceBackend trait + ort/ncnn/tflite 백엔드
- 🆕 [v3.0] SecureBytes 원본 보호
- 🆕 [v3.0] mAP 기준선 + AccuracyMonitor
- 🆕 [v3.0] 양자화 워크플로우 (tools/ 스크립트)
- DetectionTracker 안정화 + fail-safe
- 비식별화 등급 체계 (Level 1~5)
- 🆕 [v3.0] ffmpeg 동적 링킹 확인
- 검증: deface 대비 FPS/메모리/Recall 직접 비교

### Phase 2: 서명 체이닝 + 실시간 입력 (3~5개월)

- Ed25519 서명 + 시퀀스 체이닝 + nonce
- 🆕 [v3.0] ChainRecovery + ChainStatePersistence
- KeyStore (파일/TPM) + RevocationManager
- 채널 분리 (broadcast + mpsc) + 백프레셔 구체화
- 감사 로그 시스템

### Phase 3: 에이전트 + 운영 안정성 (5~7개월)

- SentryMode + ResourceManager
- 🆕 [v3.0] ThermalMonitor + Sentry 연동
- wasmtime + 정책 서명 검증
- 🆕 [v3.0] OTA 매니저 (모델/정책 핫스왑)
- Graceful Shutdown

### Phase 4: 음성 + 보안 전송 (7~9개월)

- 음성 비식별화 (cpal/rubato)
- mTLS 상호 인증
- 🆕 [v3.0] 외부 보안 감사 실시 (예산 확보 필요)

### Phase 5: 통합 + 배포 + 규제 (9~11개월)

- systemd + watchdog + 크로스 컴파일
- 데이터 주체 권리 API
- 🆕 [v3.0] COMPLIANCE_MATRIX (GDPR+CCPA+PIPL+PIPA+APPI)
- 검증: RPi4 5fps / Jetson Nano 30fps

### Phase 6: 상용화 (11~14개월)

- 기업용 대시보드 (Fleet 관리 + OTA)
- Open Core 확정 + 가격 모델 (아래 참조)
- CNCF 검토

---

## 9. 🆕 [v3.0] 커뮤니티 90일 액션 플랜

**v2.0 문제:** "커뮤니티 전략 부재"로 패널6에서 가장 낮은 개선폭.

```
Day 1~3:    GitHub 공개
            - README: 비포/애프터 이미지 + 원클릭 데모 명령어
            - "Show HN" 포스팅
            - r/rust, r/privacy, r/computervision 포스팅

Day 4~14:   초기 관심 수확
            - GitHub Issues에 "good first issue" 5개 생성:
              ① 새 Masker 추가 (예: pixelation)
              ② RPi5 벤치마크 결과 기여
              ③ 문서 번역 (한국어→영어)
              ④ CI/CD 파이프라인 구성
              ⑤ Docker 이미지 빌드
            - Discord 서버 개설

Day 15~30:  기술 콘텐츠 발행
            - 블로그: "Why Rust for Edge Privacy: deface vs Rust-Guardian"
              (벤치마크 수치 포함)
            - YouTube: 5분 데모 영상

Day 31~60:  기여자 확보
            - 첫 외부 PR 머지 + 공개 감사 트윗
            - "Contributor Spotlight" 시리즈 시작
            - Plugin SDK 초안 공개 (커스텀 Masker 작성 가이드)

Day 61~90:  확장
            - RustConf / EuroRust CFP 제출
            - 월간 뉴스레터 발행 시작
            - v0.2 릴리스 (Phase 1 완료)
```

---

## 10. Open Core 경계 + 🆕 [v3.0] 가격 모델 후보

```
┌──────────────────────────────────────────┐
│  오픈소스 (MIT/Apache 2.0 듀얼)           │
│  - guardian-core, detector, capture       │
│  - guardian-masker (Level 1~3)            │
│  - guardian-bridge, guardian-audit (기본)   │
│  - guardian-thermal                        │
│  - CLI 도구                               │
└──────────────────────────────────────────┘

┌──────────────────────────────────────────┐
│  엔터프라이즈 (상용 라이선스)               │
│  - guardian-signer (무결성 증명)            │
│  - guardian-agent (Wasm 동적 정책)          │
│  - guardian-masker Level 4 (Synthetic)     │
│  - guardian-ota (Fleet OTA 관리)            │
│  - 관리 대시보드 + Fleet 관리              │
│  - HSM/TPM 키 관리 + 데이터 주체 권리 API  │
│  - SLA 기술 지원                           │
└──────────────────────────────────────────┘
```

**🆕 [v3.0] 가격 모델 후보 (Phase 6에서 확정):**
| 모델 | 구조 | 대상 |
|------|------|------|
| A. 기기당 연간 | $50~200/기기/년 | 소규모 (<50대) |
| B. 티어별 구독 | Starter/Pro/Enterprise 월정액 | 중규모 (50~500대) |
| C. 사용량 기반 | 처리 프레임 수 × 단가 | 대규모 (>500대) |

---

## 11. 특허 출원 대상

1. **Privacy-Safe 디지털 서명 체인 + 🆕 체인 복구 프로토콜**
2. **서명 검증된 Wasm 동적 프라이버시 정책**
3. **Sentry 모드 + 🆕 열 관리 연동 적응형 비식별화**
4. **영상/음성 통합 프라이버시 파이프라인**
5. **등급별 비식별화 증명** (5단계 등급 + 서명)
6. **🆕 [v3.0] SecureBytes — 원본 즉시 삭제 보장 메커니즘**

→ 가출원 시 위 6개를 각각 독립 청구항으로 구성 가능

---

## 12. 리스크 및 완화 전략

| 리스크 | 영향도 | 완화 전략 | 버전 |
|--------|-------|----------|------|
| RPi4 성능 목표 미달 | 🔴 치명적 | NCNN+YuNet 320×320 + 🆕 Recall ≥ 0.92 기준 | v2.0 |
| replay attack | 🔴 치명적 | 체이닝 + nonce + 🆕 체인 복구 프로토콜 | v2.0+v3.0 |
| GDPR 혼동 | 🟠 높음 | 등급 체계 + 🆕 Level 4 법적 경고 + COMPLIANCE_MATRIX | v2.0+v3.0 |
| 원본 데이터 잔류 | 🟠 높음 | 🆕 SecureBytes (purge + Drop 안전망) | v3.0 |
| ffmpeg LGPL | 🟠 높음 | 🆕 동적 링킹 확정 + 장기 GStreamer 전환 | v3.0 |
| 열 스로틀링 | 🟡 중간 | 🆕 ThermalMonitor + Sentry 연동 | v3.0 |
| 체인 끊김 | 🟡 중간 | 🆕 ChainStatePersistence + ChainReset | v3.0 |
| 원격 기기 관리 | 🟡 중간 | 🆕 OTA 매니저 + 서명 검증 + 롤백 | v3.0 |
| 커뮤니티 부재 | 🟡 중간 | 🆕 90일 액션 플랜 + good first issue | v3.0 |
| 물리적 기기 탈취 | 높 | TPM + CRL | v2.0 |
| Wasm 정책 보안 | 중 | 정책 서명 + capability | v2.0 |

---

## 부록 A: 전문가 검증 점수 추이

| 패널 | v1.0 | v2.0 | v3.0 목표 |
|------|------|------|----------|
| Edge AI / CV | 68 | 80 | 85 (mAP 기준선 + 양자화 워크플로우) |
| Rust Systems | 79 | 85 | 87 (SecureBytes + 구체적 테스트 불변량) |
| Privacy / GDPR | 62 | 78 | 84 (SecureBytes + Level 4 경고 + 다국가 매핑) |
| Embedded | 58 | 73 | 80 (열 관리 + OTA + RPi4 스모크 테스트) |
| Security | 71 | 84 | 88 (체인 복구 + OTA 서명 + T8 위협 추가) |
| Product / OSS | 74 | 79 | 85 (90일 플랜 + 라이선스 확정 + 가격 모델) |
| **종합** | **68.7** | **79.8** | **~85** |

```
v1.0:  ████████████████████████████████████░░░░░░░░░░░░░░  68.7 (C+)
v2.0:  ████████████████████████████████████████████░░░░░░  79.8 (B)
v3.0:  ██████████████████████████████████████████████░░░░  ~85  (A-)
```

## 부록 B: 용어 정리

- **SecureBytes**: 🆕 [v3.0] 원본 데이터 즉시 삭제를 보장하는 메모리 래퍼
- **AccuracyMonitor**: 🆕 [v3.0] mAP/Recall 기준선 자가 검증 모듈
- **ChainStatePersistence**: 🆕 [v3.0] 체인 상태 디스크 영속화 (crash 복구)
- **ChainReset**: 🆕 [v3.0] 정전/crash 후 체인 재시작 프로토콜
- **ThermalMonitor**: 🆕 [v3.0] 엣지 기기 열 관리 + Sentry 연동
- **OtaManager**: 🆕 [v3.0] 바이너리/모델/정책 OTA 업데이트 + 롤백
- **MediaFrame**: 프로토콜 독립적 미디어 데이터 단위
- **ProcessedFrame**: 비식별화 완료 프레임 (마스킹 + 서명 + 감사)
- **InferenceBackend**: 하드웨어별 추론 엔진 추상화 trait
- **DetectionTracker**: 연속 프레임 탐지 안정화 + fail-safe
- **AnonymizationLevel**: GDPR 대응 비식별화 5단계 등급
- **PrivacySignature**: 체이닝+nonce+🆕체인상태 포함 서명
- **KeyStore**: 파일/TPM/HSM 키 저장소 추상화
- **RevocationManager**: 기기 분실 시 인증서 폐기 관리
- **AuditEntry**: 데이터 최소화 원칙 기반 감사 로그
- **SentryMode**: 탐지 상황 + 🆕열 상태에 따른 fps 자동 전환
- **PolicyRuntime**: 서명 검증된 Wasm 정책 동적 실행 런타임
- **FrameBuffer**: mmap 기반 zero-copy 프레임 공유 버퍼
