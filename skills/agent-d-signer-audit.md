# Agent D: guardian-signer + guardian-audit 개발 스킬

## 너의 역할
Rust-Guardian 프로젝트의 **무결성 증명 + 감사 시스템**을 만든다.
비식별화된 영상에 디지털 서명을 체이닝하고, 모든 처리 이력을 감사 로그로 기록한다.
**이것이 기존 비식별화 도구(deface 등)와 차별화되는 핵심 기능이다.**

## 반드시 지킬 것
- `contracts/shared_types.rs`의 `Signer` trait, `PrivacySignature`, `ChainStatus` 타입 사용
- Ed25519 서명 사용 (`ed25519-dalek` 크레이트)
- 체인 끊김 시 ChainReset으로 복구 (체인 상태 영속화 필수)
- 감사 로그는 데이터 최소화 원칙 준수 (원본 데이터 절대 기록 금지)

## 구현 대상

### guardian-signer

#### 1. Signer trait 구현 (signer.rs)
```rust
pub struct Ed25519Signer {
    signing_key: ed25519_dalek::SigningKey,
    device_id: String,
    sequence: AtomicU64,
    prev_frame_hash: Mutex<[u8; 32]>,
    chain_persistence: ChainStatePersistence,
}

#[async_trait]
impl Signer for Ed25519Signer {
    async fn sign(
        &self,
        masked_data: &[u8],
        detections: &[Detection],
        anonymization_level: AnonymizationLevel,
    ) -> Result<PrivacySignature>;
}
```

#### 2. 체인 서명 (chain.rs)
- 프레임 해시: SHA-256(masked_data)
- 시퀀스 번호: 단조 증가
- prev_frame_hash: 이전 프레임의 해시
- nonce: 랜덤 16바이트 (replay 방지)
- 서명 대상: device_id + timestamp + sequence + prev_hash + frame_hash + nonce + detection_summary + model_version + policy_hash + level

#### 3. 체인 복구 (chain_recovery.rs)
```rust
pub struct ChainRecovery;

impl ChainRecovery {
    /// 프로세스 시작 시 이전 체인 상태 복구
    pub fn recover(persistence: &ChainStatePersistence) -> ChainStartState {
        match persistence.load() {
            Ok(state) => ChainStartState::Reset {
                reason: ChainResetReason::ProcessRestart,
                previous_chain_last_seq: state.last_sequence,
                previous_chain_last_hash: state.last_frame_hash,
            },
            Err(_) => ChainStartState::Fresh,
        }
    }
}
```

#### 4. 체인 상태 영속화 (chain_persistence.rs)
- 매 프레임 서명 후 디스크에 기록 (fsync)
- 경로: `/var/lib/rust-guardian/chain_state.bin`
- crash 복구에 사용

#### 5. KeyStore (keystore.rs)
- 파일 기반 키 저장소 (Ed25519 키 쌍)
- 키 생성, 로딩, 교체
- TPM/HSM 인터페이스 (trait으로 추상화, Phase 3+)

#### 6. Verifier (verifier.rs)
- 서명 검증 + 체인 연속성 검증
- ChainReset 시 이전 체인 마지막 해시 대조

#### 7. RevocationManager (revocation.rs)
- 기기 분실 시 키 폐기 관리 (CRL)
- Phase 3+ 구현 (stub)

### guardian-audit

#### 1. AuditLogger (logger.rs)
```rust
pub struct AuditLogger {
    log_path: PathBuf,
    event_rx: EventReceiver,
}

impl AuditLogger {
    pub async fn run(&mut self) {
        while let Ok(event) = self.event_rx.recv().await {
            let entry = AuditEntry::from_event(&event);
            self.write_entry(&entry).await?;
        }
    }
}
```

#### 2. AuditEntry (entry.rs)
- 데이터 최소화: 탐지 좌표만, 원본 이미지 없음
- 구조화된 JSON 로그
- 보존 기간 자동 관리 (retention_days)

#### 3. SubjectRightsHandler (subject_rights.rs)
- GDPR Article 15~22 대응
- 데이터 주체 요청 처리 (접근, 삭제, 이동)
- Phase 5 구현 (stub)

## 의존성
`ed25519-dalek`, `sha2`, `rand` — 서명/해시/랜덤.

## guardian-core 없이 먼저 개발하는 방법
```rust
// 독립 테스트: 키 생성 → 서명 → 검증
let signer = Ed25519Signer::generate("test-device")?;
let sig = signer.sign(b"masked-data", &detections, Level1GaussianBlur).await?;
let verifier = Verifier::new(signer.public_key());
assert!(verifier.verify(&sig, b"masked-data")?);
```

## 테스트 시나리오
1. 키 생성: Ed25519 키 쌍 생성 + 저장 + 로딩
2. 서명 생성: masked_data → PrivacySignature
3. 서명 검증: Verifier로 서명 유효성 확인
4. 체이닝: 10프레임 연속 서명 → prev_hash 연속성 검증
5. 체인 복구: 강제 종료 → 재시작 → ChainReset 생성 확인
6. 변조 감지: 중간 프레임 데이터 변조 → 검증 실패 확인
7. 감사 로그: 이벤트 수신 → JSON 로그 파일 기록 확인

## 완료 기준
- `cargo test -p guardian-signer` 전부 통과
- `cargo test -p guardian-audit` 전부 통과
- 10,000프레임 체인 서명 성능: < 1ms/프레임 (Ed25519)
- 체인 끊김 → 복구 → 검증까지 end-to-end 동작
