# Agent A: guardian-core 개발 스킬

## 너의 역할
Rust-Guardian 프로젝트의 **코어 엔진**을 만든다.
Config, Pipeline 오케스트레이션, SecureBytes, Frame 관리 — 모든 크레이트가 의존하는 기반 계층.
너는 가장 먼저 시작하고, 다른 에이전트들에게 파이프라인 프레임워크를 제공한다.

## 반드시 지킬 것
- `contracts/shared_types.rs`의 타입과 trait을 정확히 사용할 것
- SecureBytes의 purge() 메커니즘을 파이프라인에서 정확히 활용할 것
- 다른 에이전트가 trait 기반으로 접근하므로 trait 시그니처 변경 금지
- 변경이 필요하면 반드시 contracts를 먼저 업데이트하고 다른 에이전트에게 알릴 것

## 구현 대상

### 1. Config (config.rs)
- `serde_yaml`로 `GuardianConfig` 로딩
- `notify` crate로 파일 변경 감지 → 핫 리로드
- 리로드 시 변경된 필드만 감지하여 해당 컴포넌트만 재시작
- 설정 검증: 누락된 필수 필드, 잘못된 값 범위 체크

### 2. SecureBytes 강화 (secure_bytes.rs)
- contracts에 정의된 SecureBytes를 래핑하는 SecureFrameBuffer
- 프레임 풀링: 메모리 재사용으로 GC 부하 감소
- purge 호출 보장: 파이프라인 완료 시 자동 purge

```rust
pub struct SecureFrameBuffer {
    pool: Vec<SecureBytes>,
    capacity: usize,
}

impl SecureFrameBuffer {
    pub fn acquire(&mut self, data: Vec<u8>) -> SecureBytes;
    pub fn release(&mut self, frame: SecureBytes);  // purge 후 풀로 반환
}
```

### 3. Pipeline (pipeline.rs)
```rust
pub struct Pipeline {
    detector: Arc<dyn InferenceBackend>,
    masker: Arc<dyn Masker>,
    signer: Option<Arc<dyn Signer>>,
    anonymization_level: AnonymizationLevel,
    event_tx: EventSender,
}

impl Pipeline {
    /// 단일 프레임 처리: 탐지 → 마스킹 → 서명
    pub async fn process_frame(&self, raw: SecureBytes) -> Result<ProcessedFrame> {
        // 1. 디코딩
        let rgb = decode_frame(raw.view().ok_or(GuardianError::Purge)?)?;
        // 2. 탐지
        let detections = self.detector.infer(&rgb).await?;
        // 3. 마스킹
        let masked = self.masker.apply(&rgb, &detections, self.anonymization_level)?;
        // 4. 원본 즉시 삭제
        raw.purge();
        drop(rgb);
        // 5. 인코딩
        let encoded = encode_frame(&masked)?;
        // 6. 서명 (선택)
        let signature = if let Some(signer) = &self.signer {
            Some(signer.sign(&encoded, &detections, self.anonymization_level).await?)
        } else {
            None
        };
        Ok(ProcessedFrame { masked_payload: encoded, detections, signature, .. })
    }
}
```

### 4. Engine (engine.rs)
- 전체 시스템 시작/중지 관리
- Capture → Pipeline → Output 연결
- Graceful Shutdown (tokio signal 처리)
- 에러 시 Graceful Degradation (Detection 실패 → 전체 블러)

### 5. Frame 유틸리티 (frame.rs)
- RGB 프레임 인코딩/디코딩 헬퍼
- 리사이즈 유틸리티 (추론 입력 크기 조정)

## 의존성 (Cargo.toml)
이미 생성됨. 필요시 수정.

## 테스트 시나리오
1. Config 로딩: YAML 파싱 → GuardianConfig 구조체
2. SecureBytes: from_raw → view → purge → view returns None
3. SecureBytes: Drop 시 자동 zeroize 확인
4. Pipeline: MockDetector + MockMasker + MockSigner → ProcessedFrame 생성
5. 핫 리로드: YAML 수정 → Config 변경 감지 이벤트 발생
6. Graceful Degradation: Detection 에러 시 전체 블러 모드 전환

## 완료 기준
- `cargo test -p guardian-core` 전부 통과
- 다른 에이전트가 `use guardian_core::contracts::*`로 모든 타입을 사용할 수 있을 것
- Pipeline이 mock 컴포넌트로 end-to-end 동작
