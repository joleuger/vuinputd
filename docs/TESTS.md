# Tests

## Manual end-to-end tests

| vuinputd | host | input type | app that creates device | app that reads device | working | Notes |
| -------------- | ---------- |---------- | ---------- |---------- |---------- |---------- |
| 0.2.0 |  Ubuntu 24.04 amd64 | virtual keyboard | Sunshine (via moonlight-qt 6.1.0 on macos) | labwc via libinput | :white_check_mark: | (1) |
| 0.2.0 |  Ubuntu 24.04  amd64| virtual mouse | Sunshine (via moonlight-qt 6.1.0 on macos) | labwc via libinput | :white_check_mark:) | (1) |
| 0.2.0 |  Ubuntu 24.04 amd64 | virtual keyboard | Steam (via Remote Play from Mac) | Return to Monkey Island | :white_check_mark: | (2) |
| 0.2.0 |  Ubuntu 24.04 amd64 | virtual gamepad | Steam (via Remote Play from Mac) | Return to Monkey Island | :x: | (2) |


(1) works also for programs running on the wayland desktop  
(2) Steam is a 32-bit application on linux