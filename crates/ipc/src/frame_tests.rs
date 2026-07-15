use super::*;
use crate::protocol::{IpcRequest, IpcResponse, IpcResult};
use soloist_core::ProcessId;

#[tokio::test]
async fn a_request_round_trips_through_the_pipe() {
    let (mut client, mut server) = tokio::io::duplex(64);
    let sent = IpcRequest::BindSessionProcess {
        process: ProcessId::from_raw(7),
    };
    write_frame(&mut client, &sent).await.expect("write");
    let got: IpcRequest = read_frame(&mut server)
        .await
        .expect("read")
        .expect("a frame, not EOF");
    assert_eq!(got, sent);
}

#[tokio::test]
async fn a_result_reply_round_trips() {
    let (mut server, mut client) = tokio::io::duplex(64);
    let reply: IpcResult = Ok(IpcResponse::Acked);
    write_frame(&mut server, &reply).await.expect("write");
    let got: IpcResult = read_frame(&mut client)
        .await
        .expect("read")
        .expect("a frame, not EOF");
    assert_eq!(got, reply);
}

#[tokio::test]
async fn a_closed_stream_reads_as_none() {
    let (client, mut server) = tokio::io::duplex(64);
    drop(client); // the peer closed before sending anything
    let got: Option<IpcRequest> = read_frame(&mut server).await.expect("read");
    assert!(got.is_none(), "a clean EOF reads as None, not an error");
}

#[tokio::test]
async fn an_oversized_prefix_is_refused_without_allocating() {
    // A length prefix beyond the cap must be rejected by the prefix alone — the reader
    // never tries to allocate the claimed size.
    let (mut writer, mut reader) = tokio::io::duplex(64);
    let bogus = MAX_FRAME + 1;
    writer
        .write_all(&bogus.to_be_bytes())
        .await
        .expect("write prefix");
    let err = read_frame::<_, IpcRequest>(&mut reader)
        .await
        .expect_err("an oversized frame must be refused");
    assert!(matches!(err, FrameError::TooLarge));
}

#[tokio::test]
async fn a_truncated_body_after_a_valid_prefix_is_an_io_error() {
    // The prefix claims more body bytes than arrive before EOF, so the frame cannot complete — an
    // unexpected end-of-stream mid-body, distinct from a clean EOF at a frame boundary (None).
    let (mut writer, mut reader) = tokio::io::duplex(64);
    writer
        .write_all(&10u32.to_be_bytes())
        .await
        .expect("write a prefix claiming 10 body bytes");
    writer
        .write_all(b"abc")
        .await
        .expect("write only part of the body");
    drop(writer); // EOF mid-body
    let err = read_frame::<_, IpcRequest>(&mut reader)
        .await
        .expect_err("a truncated body must be refused");
    assert!(matches!(err, FrameError::Io(_)));
}

#[tokio::test]
async fn a_non_json_payload_is_a_codec_error() {
    // A well-framed message whose bytes are not valid JSON fails at decode, not at framing.
    let (mut writer, mut reader) = tokio::io::duplex(64);
    let junk: &[u8] = b"not json";
    writer
        .write_all(&(junk.len() as u32).to_be_bytes())
        .await
        .expect("write the prefix");
    writer
        .write_all(junk)
        .await
        .expect("write the garbage body");
    let err = read_frame::<_, IpcRequest>(&mut reader)
        .await
        .expect_err("a non-JSON payload must be refused");
    assert!(matches!(err, FrameError::Codec(_)));
}
