#!/bin/bash
# =============================================================
# Rust-Guardian 에이전트 실행 스크립트
# =============================================================
# 사용법:
#   ./run-agents.sh a    → Agent A (core) 실행
#   ./run-agents.sh b    → Agent B (capture+output) 실행
#   ./run-agents.sh c    → Agent C (detector+masker) 실행
#   ./run-agents.sh d    → Agent D (signer+audit) 실행
#   ./run-agents.sh e    → Agent E (agent+thermal+ota) 실행
#   ./run-agents.sh f    → Agent F (api) 실행
#
# 실행 순서:
#   1단계: 터미널 1~4 동시 → ./run-agents.sh a c d f
#   2단계: core 완성 후 → ./run-agents.sh b e
# =============================================================

set -e
cd "$(dirname "$0")"

AGENT="$1"

CLAUDE_CMD="claude --dangerously-skip-permissions -p"

if [ -z "$AGENT" ]; then
  echo "사용법: ./run-agents.sh [a|b|c|d|e|f]"
  echo ""
  echo "  a  →  Agent A: guardian-core (Config, Pipeline, SecureBytes)"
  echo "  b  →  Agent B: guardian-capture + guardian-output (입출력)"
  echo "  c  →  Agent C: guardian-detector + guardian-masker (탐지+비식별화)"
  echo "  d  →  Agent D: guardian-signer + guardian-audit (서명+감사)"
  echo "  e  →  Agent E: guardian-agent + guardian-thermal + guardian-ota (런타임)"
  echo "  f  →  Agent F: guardian-api (REST API)"
  echo ""
  echo "권장 순서: (a, c, d, f 동시) → core 완성 후 (b, e 동시)"
  exit 1
fi

case "$AGENT" in
  a)
    echo "🚀 Agent A (guardian-core) 시작..."
    $CLAUDE_CMD "
당신은 Agent A입니다. Rust-Guardian 프로젝트의 코어 엔진을 만듭니다.

먼저 다음 파일들을 읽으세요:
- skills/agent-a-core.md (당신의 스킬)
- contracts/shared_types.rs (공유 타입 계약)
- contracts/mock.rs (Mock 구현)
- Rust-Guardian_System_Design.md (시스템 설계서)
- CLAUDE.md (프로젝트 규칙)

crates/guardian-core/ 디렉토리에 소스코드를 만들어주세요.
Cargo.toml은 이미 존재합니다. 수정이 필요하면 수정하세요.

구현 순서:
1. contracts를 include!로 가져오는 것 확인
2. Config 파싱 (config.rs) — serde_yaml로 GuardianConfig 로딩 + 핫 리로드
3. SecureFrameBuffer (secure_bytes.rs) — 프레임 풀링
4. Pipeline (pipeline.rs) — 탐지 → 마스킹 → 서명 오케스트레이션
5. Engine (engine.rs) — 전체 시스템 시작/중지 + Graceful Shutdown

각 단계마다 cargo test -p guardian-core가 통과하게 해주세요.
unwrap() 금지, println! 금지, tracing 사용.
"
    ;;

  b)
    echo "🚀 Agent B (guardian-capture + guardian-output) 시작..."
    $CLAUDE_CMD "
당신은 Agent B입니다. Rust-Guardian 프로젝트의 입력/출력을 만듭니다.

먼저 다음 파일들을 읽으세요:
- skills/agent-b-capture-output.md (당신의 스킬)
- contracts/shared_types.rs (공유 타입 계약)
- contracts/mock.rs (Mock 구현)
- CLAUDE.md (프로젝트 규칙)

crates/guardian-capture/와 crates/guardian-output/ 디렉토리에서 작업하세요.

구현 순서:
1. FileCapture — 이미지 파일 → SecureBytes
2. FileOutputSink — ProcessedFrame → 이미지 파일 저장
3. CaptureManager — 설정 기반 소스 선택
4. OutputManager — 다중 출력 관리
5. 단위 테스트

cargo test -p guardian-capture && cargo test -p guardian-output가 통과해야 합니다.
unwrap() 금지, println! 금지, tracing 사용.
"
    ;;

  c)
    echo "🚀 Agent C (guardian-detector + guardian-masker) 시작..."
    $CLAUDE_CMD "
당신은 Agent C입니다. Rust-Guardian 프로젝트의 탐지+비식별화를 만듭니다.
이것이 프로젝트의 핵심 기능입니다. Recall ≥ 0.92 절대 기준.

먼저 다음 파일들을 읽으세요:
- skills/agent-c-detector-masker.md (당신의 스킬)
- contracts/shared_types.rs (공유 타입 계약)
- contracts/mock.rs (Mock 구현)
- CLAUDE.md (프로젝트 규칙)

crates/guardian-detector/와 crates/guardian-masker/ 디렉토리에서 작업하세요.

구현 순서:
1. NMS 구현 (nms.rs) — Non-Maximum Suppression
2. 전처리 (preprocess.rs) — RGB 리사이즈 + 텐서 변환
3. OrtBackend (backend/ort.rs) — ONNX Runtime CPU 추론
4. DetectionTracker (tracker.rs) — 탐지 안정화 + fail-safe
5. AccuracyMonitor (accuracy_monitor.rs) — Recall 자가 검증
6. GuardianMasker — Level 1~3 비식별화 구현
7. 단위 테스트

cargo test -p guardian-detector && cargo test -p guardian-masker가 통과해야 합니다.
unwrap() 금지, println! 금지, tracing 사용.
"
    ;;

  d)
    echo "🚀 Agent D (guardian-signer + guardian-audit) 시작..."
    $CLAUDE_CMD "
당신은 Agent D입니다. Rust-Guardian 프로젝트의 무결성 증명을 만듭니다.
디지털 서명 체이닝은 기존 비식별화 도구와 차별화되는 핵심입니다.

먼저 다음 파일들을 읽으세요:
- skills/agent-d-signer-audit.md (당신의 스킬)
- contracts/shared_types.rs (공유 타입 계약)
- contracts/mock.rs (Mock 구현)
- CLAUDE.md (프로젝트 규칙)

crates/guardian-signer/와 crates/guardian-audit/ 디렉토리에서 작업하세요.

구현 순서:
1. KeyStore (keystore.rs) — Ed25519 키 생성/저장/로딩
2. Ed25519Signer — Signer trait 구현
3. Chain (chain.rs) — 시퀀스 체이닝 + prev_hash
4. ChainStatePersistence (chain_persistence.rs) — 디스크 영속화
5. ChainRecovery (chain_recovery.rs) — crash 복구
6. Verifier (verifier.rs) — 서명 + 체인 검증
7. AuditLogger (logger.rs) — 이벤트 기반 감사 로그
8. 단위 테스트

cargo test -p guardian-signer && cargo test -p guardian-audit가 통과해야 합니다.
unwrap() 금지, println! 금지, tracing 사용.
"
    ;;

  e)
    echo "🚀 Agent E (guardian-agent + guardian-thermal + guardian-ota) 시작..."
    $CLAUDE_CMD "
당신은 Agent E입니다. Rust-Guardian 프로젝트의 지능형 런타임을 만듭니다.
엣지 기기에서 장시간 안정 운영이 목표입니다.

먼저 다음 파일들을 읽으세요:
- skills/agent-e-agent-runtime.md (당신의 스킬)
- contracts/shared_types.rs (공유 타입 계약)
- contracts/mock.rs (Mock 구현)
- CLAUDE.md (프로젝트 규칙)

crates/guardian-agent/, crates/guardian-thermal/, crates/guardian-ota/ 에서 작업하세요.

구현 순서:
1. ThermalMonitor (thermal/monitor.rs) — 온도 모니터링 + fps 조절
2. SentryMode (agent/sentry.rs) — 적응형 fps + 열 관리 연동
3. ResourceManager (agent/resource.rs) — CPU/메모리 모니터링
4. OtaManager (ota/manager.rs) — 업데이트 확인 + 서명 검증
5. 다운로드 + 롤백 (ota/download.rs, ota/rollback.rs)
6. PolicyRuntime stub (agent/policy.rs) — Wasm은 Phase 3+
7. 단위 테스트

cargo test -p guardian-agent && cargo test -p guardian-thermal && cargo test -p guardian-ota
unwrap() 금지, println! 금지, tracing 사용.
"
    ;;

  f)
    echo "🚀 Agent F (guardian-api) 시작..."
    $CLAUDE_CMD "
당신은 Agent F입니다. Rust-Guardian 프로젝트의 REST API 서버를 만듭니다.

먼저 다음 파일들을 읽으세요:
- skills/agent-f-api.md (당신의 스킬)
- contracts/shared_types.rs (공유 타입 계약)
- contracts/mock.rs (Mock 구현)
- CLAUDE.md (프로젝트 규칙)

crates/guardian-api/ 디렉토리에서 작업하세요.

guardian-core의 contracts를 참조하되 독립 빌드하세요.

구현 순서:
1. AppState 정의 (state.rs) — EventBus + Config
2. axum 라우터 구성 (routes.rs)
3. 헬스체크 (handler/health.rs) — GET /api/v1/health
4. 상태 핸들러 (handler/status.rs) — GET /api/v1/status
5. 이벤트 SSE (handler/events.rs) — GET /api/v1/events/stream
6. 설정 핸들러 (handler/config.rs) — GET/PATCH /api/v1/config
7. EventStore (store.rs) — 인메모리 링 버퍼
8. 단위 테스트

cargo test -p guardian-api가 통과해야 합니다.
unwrap() 금지, println! 금지, tracing 사용.
"
    ;;

  *)
    echo "❌ 알 수 없는 에이전트: $AGENT"
    echo "사용법: ./run-agents.sh [a|b|c|d|e|f]"
    exit 1
    ;;
esac
