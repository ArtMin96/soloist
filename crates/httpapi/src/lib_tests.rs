use super::bind_loopback;

/// Two consecutive binds must land on different ports: the first holds its port, so the
/// second has to fall back off it. This holds whatever the absolute numbers are — even if
/// the preferred port was already taken or everything fell through to an OS-assigned port.
#[tokio::test]
async fn a_second_bind_falls_back_off_the_first() {
    let first = bind_loopback().await.expect("first bind");
    let first_port = first.local_addr().expect("first addr").port();
    assert!(first.local_addr().expect("first addr").ip().is_loopback());

    let second = bind_loopback().await.expect("second bind");
    let second_port = second.local_addr().expect("second addr").port();

    assert_ne!(
        first_port, second_port,
        "the second bind must not reuse the port the first is holding"
    );
}
