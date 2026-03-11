# Rust-Guardian — 병렬 개발 가이드

> 6개 에이전트가 동시에 개발하기 위한 운영 매뉴얼

---

## 1. 전체 구조 요약

```
                    ┌─────────────────────────────────────┐
                    │        contracts/shared_types.rs     │
                    │  (모든 에이전트가 공유하는 타입/trait)  │
                    └──────────────┬──────────────────────┘
                                   │
        ┌──────────────────────────┼──────────────────────────┐
        │              │           │           │              │
  ┌─────▼─────┐ ┌─────▼────┐ ┌───▼───┐ ┌────▼─────┐ ┌──────▼─────┐
  │  Agent A   │ │ Agent B  │ │Agent C│ │ Agent D  │ │  Agent E   │
  │  Core      │ │ Capture  │ │Detect │ │ Signer   │ │  Policy    │
  │ (기반계층) │ │ +Output  │ │+Mask  │ │ +Audit   │ │ +Thermal   │
  └─────┬─────┘ └──────────┘ └───────┘ └──────────┘ │ +OTA       │
        │                                            └────────────┘
        │
  ┌─────▼─────┐
  │  Agent F   │
  │  API       │
  └────────────┘
```

---

## 2. 에이전트 역할 배정

| 에이전트 | 크레이트 | 핵심 역할 | 스킬 파일 |
|----------|----------|----------|-----------|
| **Agent A** | `guardian-core` | Config, Pipeline, SecureBytes, Engine | `skills/agent-a-core.md` |
| **Agent B** | `guardian-capture`, `guardian-output` | 입력 소스 + 출력 관리 | `skills/agent-b-capture-output.md` |
| **Agent C** | `guardian-detector`, `guardian-masker` | 탐지 + 비식별화 | `skills/agent-c-detector-masker.md` |
| **Agent D** | `guardian-signer`, `guardian-audit` | 서명 체이닝 + 감사 | `skills/agent-d-signer-audit.md` |
| **Agent E** | `guardian-agent`, `guardian-thermal`, `guardian-ota` | 정책 + 열관리 + OTA | `skills/agent-e-agent-runtime.md` |
| **Agent F** | `guardian-api` | REST API + 대시보드 | `skills/agent-f-api.md` |

---

## 3. 의존성 그래프와 병렬화 전략

```
Week 1-2:  [A: core 기본]  [C: detector 단독]  [D: signer 단독]  [F: API 목업]
               │
Week 3-4:  [A: core 완성] ──▶ [B: capture 시작]  [C: masker 통합]
               │                    │
Week 5-6:  [B: output 완성]  [D: audit 통합]  [E: thermal+sentry]  [F: SSE 통합]
               │                    │               │           │
Week 7-8:  ◀──────── 전체 통합 테스트 + 버그 수정 ────────────▶
```

### 핵심 원칙: Mock으로 독립 개발

Agent A(core)가 완성되기 전에도 B, C, D, E, F는 **mock**을 써서 동시 개발한다.

```rust
// 모든 에이전트가 사용하는 공통 mock (contracts/mock.rs)
let detector = MockInferenceBackend::face_detected();
let masker = MockMasker;
let signer = MockSigner;
let capture = MockCaptureSource::new(10);
let output = MockOutputSink;
```

**각 trait으로 인터페이스를 분리했기 때문에** mock만 교체하면 실제 구현 없이 독립 빌드/테스트 가능.

---

## 4. Claude Code 에이전트 실행 방법

### 4.1 에이전트 실행

```bash
# 터미널 1 — Agent A: Core
./run-agents.sh a

# 터미널 2 — Agent C: Detector + Masker (core 없이 독립)
./run-agents.sh c

# 터미널 3 — Agent D: Signer + Audit (core 없이 독립)
./run-agents.sh d

# 터미널 4 — Agent F: API (core 없이 독립)
./run-agents.sh f

# core 완성 후
# 터미널 5 — Agent B: Capture + Output
./run-agents.sh b

# 터미널 6 — Agent E: Agent + Thermal + OTA
./run-agents.sh e
```

---

## 5. 통합 순서

### Phase 1: Core + Detector + Masker (프라이버시 파이프라인)
```bash
# 검증: 이미지 1장 → 얼굴 탐지 → 블러 처리 → 출력
rust-guardian blur input.jpg output.jpg
```

### Phase 2: Core + Signer (무결성 증명)
```bash
# 비식별화된 이미지 + 서명 생성
# 체인 서명 검증
```

### Phase 3: Core + Capture + Output (실시간 파이프라인)
```bash
# RTSP → 실시간 비식별화 → 파일/스트림 출력
```

### Phase 4: 전체 통합
```
Capture → Pipeline → Detector → Masker → Signer → Output
                                                └→ AuditLog
                                                └→ API SSE
```

---

## 6. 충돌 방지 규칙

### 6.1 파일 소유권

| 디렉토리 | 소유 에이전트 | 다른 에이전트 접근 |
|----------|-------------|-----------------|
| `crates/guardian-core/` | Agent A | 읽기만 |
| `crates/guardian-capture/`, `crates/guardian-output/` | Agent B | 읽기만 |
| `crates/guardian-detector/`, `crates/guardian-masker/` | Agent C | 읽기만 |
| `crates/guardian-signer/`, `crates/guardian-audit/` | Agent D | 읽기만 |
| `crates/guardian-agent/`, `crates/guardian-thermal/`, `crates/guardian-ota/` | Agent E | 읽기만 |
| `crates/guardian-api/` | Agent F | 읽기만 |
| `contracts/` | **공동 소유** | 변경 시 PR 필수 |

### 6.2 contracts 변경 프로토콜

1. 변경이 필요한 에이전트가 `contracts/shared_types.rs` 수정 PR 생성
2. PR 설명에 "영향받는 에이전트: B, D" 등 명시
3. 다른 에이전트가 확인 후 자기 크레이트 업데이트
4. 모든 크레이트 `cargo test` 통과 확인 후 merge

### 6.3 Git 브랜치 전략

```
main ─────────────────────────────────────────▶
  │
  ├── agent-a/core ────── Agent A 작업 ──── PR → main
  ├── agent-b/capture ─── Agent B 작업 ──── PR → main
  ├── agent-c/detector ── Agent C 작업 ──── PR → main
  ├── agent-d/signer ──── Agent D 작업 ──── PR → main
  ├── agent-e/runtime ─── Agent E 작업 ──── PR → main
  └── agent-f/api ─────── Agent F 작업 ──── PR → main
```

---

## 7. Phase 0 체크리스트 (2주 MVP)

- [ ] contracts/shared_types.rs 확정
- [ ] Cargo workspace 구조 생성
- [ ] Agent A: Config 파싱 + Pipeline stub
- [ ] Agent C: ORT + YuNet 얼굴 탐지 + Gaussian Blur
- [ ] Agent B: FileCapture + FileOutputSink
- [ ] CLI: `rust-guardian blur input.jpg output.jpg` 동작
- [ ] Agent D: Ed25519 서명 + 체이닝 기본
- [ ] Agent F: /api/v1/health 응답

---

## 8. 트러블슈팅

### "다른 에이전트의 크레이트가 컴파일 안 돼요"
→ 자기 크레이트만 빌드: `cargo build -p guardian-detector`
→ 다른 크레이트 의존은 mock으로 대체

### "contracts 타입이 부족해요"
→ contracts 변경 PR 생성 → 다른 에이전트에게 리뷰 요청
→ 임시로 자기 크레이트 내부에 확장 타입 정의

### "ort 빌드가 안 돼요" (Agent C)
→ 시스템에 ONNX Runtime 설치 필요:
```bash
# macOS
brew install onnxruntime
# Ubuntu
apt install libonnxruntime-dev
```

### "SecureBytes purge가 안 되는 것 같아요"
→ `is_purged()` 체크 → purge 후 view() 반환값 확인
→ Drop 안전망이 있으므로 scope 탈출 시 자동 zeroize됨
