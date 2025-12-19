# Tests

## Integration tests

Install bubblewrap:
`apt-get install bubblewrap`.

Run with `cargo test -p vuinputd-tests --features "requires-privileges requires-uinput requires-bwrap"`.



## Performance tests

Using CUSE introduces an additional round trip between kernel and userspace, which inevitably adds overhead compared to direct uinput access. To estimate the order of magnitude of this overhead, the `vuinputd-tests` include a simple integration test that emits two input events: once using direct uinput access and once via `vuinputd` v0.3.

The test measures the elapsed time between emitting an event and receiving it again, using `CLOCK_MONOTONIC`. The measured latencies were:

* **First call:** 16 µs (direct uinput) vs. 90 µs (vuinputd)
* **Second call:** 7 µs (direct uinput) vs. 68 µs (vuinputd)

As expected, `vuinputd` introduces a clearly measurable performance penalty due to the additional userspace round trip. However, even in this best-case microbenchmark, the absolute overhead remains well below 0.1 ms per event.

In practical terms, this level of overhead is negligible for real-world usage. For latency-sensitive applications such as gaming, tens of microseconds are several orders of magnitude smaller than typical sources of latency such as frame rendering time, compositor delays, scheduling jitter, or network latency. Even a single rendered frame at 60 Hz already accounts for roughly 16.6 ms, making the additional cost introduced by `vuinputd` effectively unobservable to the user.

It is important to note that this benchmark is intentionally minimal and primarily intended to provide a rough sense of scale. It does not model realistic workloads, higher event rates, or concurrent inputs. More comprehensive benchmarks are required to assess behavior under load and contention. Nevertheless, these results demonstrate that the architectural approach taken by `vuinputd` is sound and does not introduce prohibitive latency by design.
As long as more realistic benchmarks confirm similar behavior under load, `vuinputd` can be considered suitable even for interactive and latency-sensitive use cases.

Detailed results:

`integration_tests.rs#test_keyboard_in_container_with_uinput`:  
```
{"events":[{"tv_sec":3133476,"tv_nsec":947503794,"duration_usec":16,"type_":1,"code":57,"value":1,"send_and_receive_match":true},{"tv_sec":3133476,"tv_nsec":947520555,"duration_usec":7,"type_":1,"code":57,"value":0,"send_and_receive_match":true}]}
```

`integration_tests.rs#test_keyboard_in_container_with_vuinput`: 
```
Event log: {"events":[{"tv_sec":3133303,"tv_nsec":796108454,"duration_usec":90,"type_":1,"code":57,"value":1,"send_and_receive_match":true},{"tv_sec":3133303,"tv_nsec":796198973,"duration_usec":68,"type_":1,"code":57,"value":0,"send_and_receive_match":true}]}
```

## Manual end-to-end tests

| vuinputd | host | input type | app that creates device | app that reads device | working | Notes |
| -------------- | ---------- |---------- | ---------- |---------- |---------- |---------- |
| 0.2.0 |  Ubuntu 24.04 amd64 | virtual keyboard | Sunshine (via moonlight-qt 6.1.0 on macos) | labwc via libinput | :white_check_mark: | (1) |
| 0.2.0 |  Ubuntu 24.04  amd64| virtual mouse | Sunshine (via moonlight-qt 6.1.0 on macos) | labwc via libinput | :white_check_mark:) | (1) |
| 0.2.0 |  Ubuntu 24.04 amd64 | virtual keyboard | Steam (via Remote Play from Mac) | Return to Monkey Island | :white_check_mark: | (2) |
| 0.2.0 |  Ubuntu 24.04 amd64 | virtual gamepad | Steam (via Remote Play from Mac) | Return to Monkey Island | :x: | (2) |


(1) works also for programs running on the wayland desktop  
(2) Steam is a 32-bit application on linux