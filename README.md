<!-- Improved compatibility of back to top link: See: https://github.com/othneildrew/Best-README-Template/pull/73 -->
<a name="readme-top"></a>
<!--
Readme template from https://github.com/othneildrew/Best-README-Template
-->

<!-- PROJECT SHIELDS -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->

[//]: # ([![Contributors][contributors-shield]][contributors-url])
[//]: # ([![Forks][forks-shield]][forks-url])
[//]: # ([![Stargazers][stars-shield]][stars-url])
[![GPL3 License][license-shield]][license-url]
[![Issues][issues-shield]][issues-url]
![build-shield]

<div align="center">
<!-- 
  <a href="https://github.com/github_username/repo_name">
    <img src="images/logo.png" alt="Logo" width="80" height="80">
  </a>
-->

<h1 align="center">GrowattServer</h1>

  <p align="center">
    A server to intercept Growatt Inverters and Smart Energy Meters messages in order to use them locally.
    <br />
    <!--<a href="https://github.com/github_username/repo_name"><strong>Explore the docs »</strong></a>-->
    <br />
    <a href="https://github.com/github_username/repo_name/issues">Report Bug</a>
    ·
    <a href="https://github.com/github_username/repo_name/issues">Request Feature</a>
  </p>
</div>

## About The Project

[//]: # ([![Product Name Screen Shot][product-screenshot]]&#40;https://example.com&#41;)

This project aims to allow Growatt Inverters owners to take full ownership of their systems.
This can be for various reasons; at first it was conceived to be able to get data from the inverters with as little changes as possible to the system.

Since the inverters by themselves do not have the capability to connect to the internet, they need a middleman to query them and report the data online.
There are excellent projects that replace the middleman with a custom device that can interrogate the inverter(s) via RS485, however it may not be the best solution, especially if the system is made of multiple inverters.

In this case, the other option is to intercept the messages of the middleman and act as a second one ourselves.
This way, the messages are decoded, and sent via MQTT or similar means (planned) to whatever system that may consume the data.

Currently, the project allows for the messages exchanged between the system and Growatt's server to be forwarded, but it is planned to be able to take it offline for complete control. 

It is known that there are different protocol versions used by Growatt devices. 
Right now the project can "understand" Protocol V6, however as soon as data from other protocol versions will be available, the project will be updated to support them.

## Getting Started

Given the current project status, no standalone runtime is available for download, but they're planned for the near future.

Right now the only way available is to pull the repository and compile it.
There is a docker image, but it is currently updated only during testing, so to follow the latest version right now it is suggested to pull directly from the repository.

It is planned to supply executables (GNU/Linux only) and a Docker image.

### Prerequisites

Once the executable will be released here on GitHub, it will be sufficient to download it and run it alongside the `inverters` configuration directory, which contains mappings for the data returned by the inverters.

### Installation

#### From sources

I expect you to know what you're doing. You'll need to:

1. Pull the repository:
    ```shell
   git clone https://github.com/stefa168/growatt_server.git
   ```
2. [Install the Rust toolchain](https://www.rust-lang.org/tools/install)
3. ```shell
    cd growatt_server
    cargo install --path .
    ```

#### Docker

You can use the following compose sample:

```yaml
version: "3.9"
services:
  growatt_server:
    image: stefa168/growatt_server:latest
    build: .
    ports:
      - "5279:5279/tcp"
    volumes:
      - ./inverters:/usr/local/bin/inverters
      - config.yaml:/usr/local/bin/config.yaml
    environment:
      LOG_LEVEL: INFO
```

---

Please note that the `inverters` folder is mandatory, and must contain the (currently only) mapping file.

<!-- USAGE EXAMPLES -->
## Usage

To start the server it is simply a matter of running the executable.
By default, it looks for a configuration file in the same directory, however it can be changed with the `-c` or `--config_path` optional parameter.

Please take a look at the default [configuration file for more information](config.yaml). (Soon they will be listed here too)

If a log level different from `INFO` is necessary, set the environment variable `LOG_LEVEL` to the required level (`DEBUG`, `TRACE`, etc.)

The server will relay data to the endpoint specified in the configuration file.
It defaults to Growatt's servers on `server.growatt.com`.

For more command-line options, use the `--help` option.

<!-- ROADMAP -->
## Roadmap

- [ ] Message interception
    - [ ] Proxy
        - [x] Basic proxy
        - [ ] Proxy with filtering features (for unwanted remote control)
    - [ ] Impersonator
    - Protocols
        - [x] Protocol v6
        - [ ] ?
- [ ] Data
    - [x] Storage
    - [ ] MQTT
    - [ ] Home Assistant
- [ ] Frontend

See the [open issues](https://github.com/github_username/repo_name/issues) for a full list of proposed features (and known issues).

<!-- CONTRIBUTING -->
## Contributing

Contributions are vital to the Open Source ecosystem. 
If you have any suggestion or improvement, please submit it!

You can open an issue, or fork the repository and then make a pull request with your new features and suggestions.

The commit messages are expected to follow the [Conventional Commits format](https://www.conventionalcommits.org/en/v1.0.0/).

<!-- LICENSE -->
## License

Distributed under the GNU GPL3 License. See `LICENSE.md` for more information.

<!-- CONTACT -->
## Contact

Stefano Vittorio Porta - [@stefa168](https://twitter.com/stefa168)

Project Link: [https://github.com/stefa168/growatt_server](https://github.com/stefa168/growatt_server)





<!-- ACKNOWLEDGMENTS -->
## Acknowledgments

* [Protocol Analysis, thanks to Johan Vromans' article](https://www.vromans.org/software/sw_growatt_wifi_protocol.html)
* [NRG, by Jeroen Roos, for some concepts regarding the deobfuscation of messages](https://gitlab.com/jeroenrnl/nrg)
* [PyGrowatt](https://github.com/aaronjbrown/PyGrowatt)
* [Readme Template](https://github.com/othneildrew/Best-README-Template)





<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[issues-shield]: https://img.shields.io/github/issues/stefa168/growatt_server.svg?logo=github
[issues-url]: https://github.com/stefa168/growatt_server/issues
[license-shield]: https://www.gnu.org/graphics/gplv3-or-later-sm.png
[license-url]: https://github.com/stefa168/growatt_server/blob/master/LICENSE.md
[build-shield]: https://img.shields.io/github/actions/workflow/status/stefa168/growatt_server/rust.yml?logo=rust
