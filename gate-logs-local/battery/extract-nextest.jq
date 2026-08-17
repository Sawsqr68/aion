select(.type == "test" and .event == "failed")
| {
    type: "nextest",
    test: .name,
    stdout: (.stdout // ""),
    message: "test failed"
  }
