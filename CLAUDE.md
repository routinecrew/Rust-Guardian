# Rust-Guardian — Claude Code 지침

## 프로젝트 개요
Edge Privacy Filter — 엣지 기기에서 실시간으로 얼굴/번호판을 탐지하고 비식별화하는 Rust 프로젝트.
디지털 서명 체이닝으로 무결성을 증명하고, Wasm 동적 정책으로 규정을 준수한다.

## 반드시 읽어야 할 파일 (우선순위 순)
1. `contracts/shared_types.rs` — 공유 타입/trait. **절대 임의 변경 금지.**
2. `contracts/mock.rs` — 독립 개발용 mock 구현
3. `Rust-Guardian_System_Design.md` — 전체 아키텍처 설계서
4. `PARALLEL_DEV_GUIDE.md` — 병렬 개발 운영 가이드
5. `skills/agent-*.md` — 본인 담당 에이전트의 상세 스킬

## 에이전트 배정
| 에이전트 | 크레이트 | 역할 |
|----------|----------|------|
| Agent A | `crates/guardian-core/` | Config, Pipeline, SecureBytes, Engine |
| Agent B | `crates/guardian-capture/`, `crates/guardian-output/` | 입력 소스 + 출력 관리 |
| Agent C | `crates/guardian-detector/`, `crates/guardian-masker/` | 탐지 + 비식별화 |
| Agent D | `crates/guardian-signer/`, `crates/guardian-audit/` | 서명 체이닝 + 감사 |
| Agent E | `crates/guardian-agent/`, `crates/guardian-thermal/`, `crates/guardian-ota/` | 정책 + 열관리 + OTA |
| Agent F | `crates/guardian-api/` | REST API + 대시보드 |

## 코딩 규칙
- `unwrap()` 금지. 모든 에러는 `anyhow::Result`로 전파
- `println!` 금지. 로그는 `tracing` 크레이트 사용
- `unsafe` 사용 시 반드시 주석으로 안전성 근거 명시
- public API에는 doc comment 필수
- 테스트: 각 public 함수에 최소 1개 단위 테스트
- 원본 프레임은 반드시 `SecureBytes`로 래핑하고 처리 후 `purge()` 호출

## 빌드/테스트
```bash
# 전체 빌드
cargo build --workspace

# 전체 테스트
cargo test --workspace

# 특정 크레이트만 테스트
cargo test -p guardian-core
cargo test -p guardian-capture
cargo test -p guardian-detector
cargo test -p guardian-masker
cargo test -p guardian-signer
cargo test -p guardian-audit
cargo test -p guardian-agent
cargo test -p guardian-thermal
cargo test -p guardian-ota
cargo test -p guardian-output
cargo test -p guardian-api
```

## contracts 변경 절차
1. 변경 필요성 설명과 함께 PR 생성
2. 영향받는 에이전트 목록 명시
3. 모든 크레이트 `cargo test --workspace` 통과 확인 후 merge

## 독립 개발 방법
core가 아직 없어도 `contracts/mock.rs`의 Mock 구현을 사용하면
각 크레이트를 독립적으로 빌드하고 테스트할 수 있다.
통합 시에만 mock → 실제 구현으로 교체.

## Token Optimization
- **서브에이전트(Agent tool) 사용 금지** — 직접 Glob, Grep, Read 등 기본 도구로 해결할 것
- **응답은 최소한으로** — 코드 변경 시 변경 사항만 간결히 설명
- **파일은 필요한 부분만 읽기** — offset/limit 활용
- **병렬 도구 호출 활용** — 독립적인 호출은 한 번에 병렬 실행
