//! 프레임 처리 파이프라인 오케스트레이션
//!
//! CaptureSource → Detector → Tracker → Masker → Signer → OutputSink
//! 각 단계는 trait으로 추상화되어 있어 mock으로 교체 가능.

// TODO: Agent A가 구현
