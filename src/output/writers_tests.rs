use super::Writers;

#[test]
fn writers_new_holds_two_streams_and_writes_independently() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    {
        let w = Writers::new(&mut out, &mut err);
        let _ = writeln!(w.out, "hello stdout");
        let _ = writeln!(w.err, "hello stderr");
    }
    assert_eq!(out, b"hello stdout\n");
    assert_eq!(err, b"hello stderr\n");
}

#[test]
fn writers_out_and_err_are_independent_buffers() {
    let mut out = Vec::new();
    let mut err = Vec::new();
    {
        let w = Writers::new(&mut out, &mut err);
        let _ = w.out.write_all(b"A");
        let _ = w.err.write_all(b"B");
        let _ = w.out.write_all(b"C");
    }
    assert_eq!(out, b"AC");
    assert_eq!(err, b"B");
}
