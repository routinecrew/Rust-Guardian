# Agent B: guardian-capture + guardian-output 개발 스킬

## 너의 역할
Rust-Guardian 프로젝트의 **입력 소스 관리**와 **출력 관리**를 만든다.
카메라/RTSP/파일에서 프레임을 가져오고, 비식별화된 결과를 파일/스트림/전송으로 내보낸다.
파이프라인의 시작과 끝을 담당한다.

## 반드시 지킬 것
- `contracts/shared_types.rs`의 `CaptureSource`, `OutputSink` trait을 구현할 것
- 원본 프레임은 반드시 `SecureBytes`로 래핑해서 전달할 것
- 출력에는 비식별화된 `ProcessedFrame`만 사용할 것 (원본 접근 금지)
- 캡처 실패 시 에러를 전파하고, 재시도 로직은 Engine이 담당

## 구현 대상

### guardian-capture

#### 1. FileCapture (file.rs)
```rust
pub struct FileCapture {
    path: PathBuf,
}

impl FileCapture {
    pub fn new(path: PathBuf) -> Result<Self>;
}

#[async_trait]
impl CaptureSource for FileCapture {
    async fn next_frame(&mut self) -> Result<Option<SecureBytes>>;
    fn name(&self) -> &str { "file" }
    async fn stop(&mut self) -> Result<()>;
}
```
- 이미지 파일 (jpg, png) → RGB 변환 → SecureBytes
- Phase 0 MVP에서 가장 먼저 필요

#### 2. RtspCapture (rtsp.rs)
- `retina` 크레이트로 RTSP 스트림 수신
- H264 NAL unit → SecureBytes
- 연결 끊김 시 자동 재연결 (backoff)

#### 3. V4l2Capture (v4l2.rs)
- V4L2 카메라 입력 (Linux 전용)
- `#[cfg(target_os = "linux")]` 게이트
- 하드웨어 디코더 연동 가능

#### 4. CaptureManager (manager.rs)
- 설정에 따라 적절한 CaptureSource 생성
- 소스 전환 지원

### guardian-output

#### 1. FileOutputSink (file.rs)
```rust
pub struct FileOutputSink {
    output_dir: PathBuf,
}

#[async_trait]
impl OutputSink for FileOutputSink {
    async fn write(&self, frame: &ProcessedFrame) -> Result<()>;
    fn name(&self) -> &str { "file" }
    async fn stop(&self) -> Result<()>;
}
```
- 비식별화된 프레임을 이미지/비디오 파일로 저장
- 시퀀스 번호 기반 파일명

#### 2. RtspOutputSink (rtsp.rs)
- 비식별화된 스트림을 RTSP로 재전송

#### 3. OutputManager (manager.rs)
- 다중 출력 대상 동시 지원
- 설정에 따라 OutputSink 생성

## 의존성
이미 생성됨. 필요시 수정.
`image` 크레이트로 이미지 파일 처리.

## guardian-core 없이 먼저 개발하는 방법
```rust
// mock으로 독립 테스트
let capture = FileCapture::new("test.jpg".into())?;
let frame = capture.next_frame().await?;
assert!(frame.is_some());
let secure = frame.unwrap();
assert!(secure.view().is_some());
```

## 테스트 시나리오
1. FileCapture: 테스트 이미지 → SecureBytes 생성 확인
2. FileOutputSink: ProcessedFrame → 파일 저장 확인
3. CaptureManager: 설정 기반 소스 생성
4. SecureBytes: 캡처 후 view() 정상, purge 후 None

## 완료 기준
- `cargo test -p guardian-capture` 전부 통과
- `cargo test -p guardian-output` 전부 통과
- FileCapture + FileOutputSink로 이미지 입력 → 이미지 출력 파이프라인 동작
