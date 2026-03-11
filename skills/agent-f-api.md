# Agent F: guardian-api 개발 스킬

## 너의 역할
Rust-Guardian 프로젝트의 **REST API 서버 + 모니터링 대시보드**를 만든다.
운영자가 시스템 상태를 보고, 설정을 변경하고, 이벤트를 모니터링할 수 있게 한다.
데이터 주체 권리 API도 제공한다 (GDPR Article 15~22).

## 반드시 지킬 것
- `contracts/shared_types.rs`의 타입을 API 응답에 그대로 사용
- `EventReceiver`로 시스템 이벤트를 수신
- API는 `axum`, 실시간 이벤트는 SSE(Server-Sent Events)
- 원본 데이터를 API로 노출하지 말 것 (ProcessedFrame만)

## 구현 대상

### 1. REST API 엔드포인트

#### 시스템
```
GET  /api/v1/health           → 헬스체크
GET  /api/v1/metrics          → Prometheus 형식 메트릭
GET  /api/v1/status           → 시스템 상태 (fps, CPU, 메모리, 온도)
```

#### 설정
```
GET   /api/v1/config          → 현재 설정 반환
PATCH /api/v1/config          → 설정 부분 업데이트 → 핫 리로드 트리거
```

#### 이벤트
```
GET  /api/v1/events           → 최근 이벤트 목록 (페이지네이션)
GET  /api/v1/events/stream    → SSE 실시간 이벤트 스트림
GET  /api/v1/events/stats     → 이벤트 통계
```

#### 서명 검증
```
POST /api/v1/verify           → 서명 체인 검증 요청
GET  /api/v1/chain/status     → 현재 체인 상태
```

#### 데이터 주체 권리 (Phase 5)
```
POST /api/v1/subject/access   → 데이터 접근 요청
POST /api/v1/subject/delete   → 데이터 삭제 요청
GET  /api/v1/subject/status   → 요청 처리 상태
```

### 2. SSE 실시간 이벤트 (sse.rs)
```rust
async fn event_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.event_bus.subscribe();
    let stream = async_stream::stream! {
        while let Ok(event) = rx.recv().await {
            let json = serde_json::to_string(&event).unwrap_or_default();
            yield Ok(Event::default().data(json));
        }
    };
    Sse::new(stream)
}
```

### 3. AppState (state.rs)
```rust
pub struct AppState {
    pub event_bus: EventSender,
    pub config: Arc<RwLock<GuardianConfig>>,
    pub event_store: Arc<EventStore>,
}
```

### 4. EventStore (store.rs)
- 인메모리 링 버퍼 (최근 10,000건)
- 타입별, 시간대별 인덱스
- 페이지네이션: cursor 기반

### 5. 대시보드 (static/)
- 초기: axum에서 정적 HTML 서빙
- 실시간 이벤트 피드 (SSE)
- 시스템 상태 모니터링

## 의존성
`axum`, `tower`, `tower-http`, `serde_json`, `async-stream`

## guardian-core 없이 먼저 개발하는 방법
```rust
pub fn mock_app_state() -> AppState {
    let (event_tx, _) = new_event_bus();
    // 가짜 이벤트를 주기적으로 발행
    let tx = event_tx.clone();
    tokio::spawn(async move {
        loop {
            let _ = tx.send(GuardianEvent::SystemStatus {
                fps: 10.0, cpu_usage: 35.0, memory_mb: 256.0, temperature: Some(55.0),
            });
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });
    AppState {
        event_bus: event_tx,
        config: Arc::new(RwLock::new(test_config())),
        event_store: Arc::new(EventStore::new(10_000)),
    }
}
```

## 테스트 시나리오
1. GET /api/v1/health → 200 OK
2. GET /api/v1/status → 시스템 상태 JSON
3. GET /api/v1/events → 빈 배열 (이벤트 없을 때)
4. SSE 연결 → 가짜 이벤트 수신 확인
5. PATCH /api/v1/config → 설정 변경 반영 확인
6. EventStore: 10,000건 초과 시 오래된 이벤트 자동 삭제

## 완료 기준
- `cargo test -p guardian-api` 전부 통과
- 모든 API 엔드포인트 정상 응답
- SSE로 실시간 이벤트 수신 동작
- `curl http://localhost:9090/api/v1/health` 성공
