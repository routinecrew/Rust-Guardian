# Agent E: guardian-agent + guardian-thermal + guardian-ota 개발 스킬

## 너의 역할
Rust-Guardian 프로젝트의 **지능형 런타임**을 만든다.
동적 정책 실행, Sentry 모드 (적응형 fps), 열 관리, OTA 업데이트를 담당한다.
엣지 기기에서 장시간 안정적으로 운영되게 하는 것이 목표다.

## 반드시 지킬 것
- `contracts/shared_types.rs`의 이벤트 버스(`EventSender/EventReceiver`)를 통해 시스템과 통신
- 열 관리: critical 온도 초과 시 반드시 Standby 전환 (안전 우선)
- OTA: 업데이트 패키지 서명 검증 필수 (미검증 패키지 적용 절대 금지)
- Wasm 정책은 wasmtime 샌드박스 내에서만 실행

## 구현 대상

### guardian-agent

#### 1. SentryMode (sentry.rs)
```rust
pub struct SentryMode {
    state: SentryState,
    config: SentryConfig,
    thermal_monitor: Option<Arc<ThermalMonitor>>,
}

pub enum SentryState {
    Active { fps: u32 },    // 탐지 활발 → 고fps
    Idle { fps: u32 },      // 탐지 없음 → 저fps (전력 절감)
    Standby,                 // 열 위험 → 최소 동작
}

impl SentryMode {
    /// 탐지 결과에 따라 상태 전환
    pub fn update(&mut self, detection_count: usize) -> u32 {
        // Active → Idle: N초간 탐지 없음
        // Idle → Active: 탐지 발생
        // * → Standby: thermal critical
        self.recommend_fps()
    }
}
```

#### 2. PolicyRuntime (policy.rs)
- wasmtime 기반 Wasm 정책 실행
- 정책 서명 검증 후에만 로딩
- capability 제한: 파일시스템/네트워크 접근 차단
- 핫스왑: 새 정책 로딩 → 검증 → 교체

```rust
pub struct PolicyRuntime {
    engine: wasmtime::Engine,
    current_policy: Option<LoadedPolicy>,
}

impl PolicyRuntime {
    pub fn load_policy(&mut self, wasm_bytes: &[u8], signature: &[u8]) -> Result<()>;
    pub fn evaluate(&self, detections: &[Detection]) -> Result<PolicyDecision>;
}
```

#### 3. ResourceManager (resource.rs)
- CPU/메모리 사용량 모니터링
- 백프레셔 메커니즘
- 리소스 부족 시 품질 저하 (해상도 ↓, fps ↓)

### guardian-thermal

#### 1. ThermalMonitor (monitor.rs)
```rust
pub struct ThermalMonitor {
    temp_path: PathBuf,          // /sys/class/thermal/thermal_zone0/temp
    warning_threshold: f32,       // 기본: 70°C
    critical_threshold: f32,      // 기본: 80°C
    event_tx: EventSender,
}

impl ThermalMonitor {
    pub fn current_temp(&self) -> f32;
    pub fn recommend_fps(&self, base_fps: u32) -> u32;
    pub async fn run(&self);     // 주기적 모니터링 루프
}
```

#### 2. x86/macOS 호환
- `/sys/class/thermal/` 없는 환경에서는 비활성화
- `#[cfg(target_os = "linux")]` 게이트

### guardian-ota

#### 1. OtaManager (manager.rs)
```rust
pub struct OtaManager {
    config: OtaConfig,
    signing_key: ed25519_dalek::VerifyingKey,
}

impl OtaManager {
    pub async fn check_updates(&self) -> Result<Vec<SignedUpdate>>;
    pub async fn apply_update(&self, update: &SignedUpdate) -> Result<()>;
}
```

#### 2. 업데이트 다운로드 (download.rs)
- reqwest로 다운로드
- SHA-256 해시 검증
- 실패 시 최대 3회 재시도

#### 3. 롤백 (rollback.rs)
- 바이너리: A/B 파티션 스왑
- 모델/정책: 이전 버전 백업 유지
- rollback_timeout 내 헬스체크 실패 시 자동 롤백

## 의존성
guardian-agent: wasmtime (Phase 3+, 초기에는 stub)
guardian-thermal: 시스템 의존성 없음
guardian-ota: reqwest, ed25519-dalek, sha2

## guardian-core 없이 먼저 개발하는 방법
```rust
// ThermalMonitor 독립 테스트 (mock temp file)
let monitor = ThermalMonitor::new_with_path(
    "/tmp/mock_temp",
    70.0, 80.0,
    event_tx,
);
std::fs::write("/tmp/mock_temp", "75000")?;  // 75°C
assert_eq!(monitor.recommend_fps(30), 15);   // 절반
```

## 테스트 시나리오
1. SentryMode: 탐지 있음 → Active, 5초 무탐지 → Idle
2. ThermalMonitor: 75°C → fps 절반, 85°C → Standby
3. OTA 서명 검증: 유효 서명 → 수락, 변조 → 거부
4. OTA 롤백: 적용 → 헬스체크 실패 시뮬레이션 → 롤백
5. PolicyRuntime: Wasm 정책 로딩 → 평가 → 결과 확인 (Phase 3+)

## 완료 기준
- `cargo test -p guardian-agent` 전부 통과
- `cargo test -p guardian-thermal` 전부 통과
- `cargo test -p guardian-ota` 전부 통과
- SentryMode + ThermalMonitor 연동 동작
- OTA 서명 검증 → 다운로드 → 적용 → 롤백 end-to-end
