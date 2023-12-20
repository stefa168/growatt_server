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
![Docker Image Version (latest semver)](https://img.shields.io/docker/v/stefa168/growatt_server?sort=semver&logo=docker)


<div align="center">

<h1 align="center">GrowattServer</h1>

  <p align="center">
    Intercept messages from Growatt inverters and use them locally! 
    <br />
    <!--<a href="https://github.com/stefa168/growatt_server"><strong>Explore the docs »</strong></a>-->
    <br />
    <a href="https://github.com/stefa168/growatt_server/issues">Report Bug</a>
    ·
    <a href="https://github.com/stefa168/growatt_server/issues">Request Feature</a>
    ·
    <a href="https://github.com/stefa168/growatt_server/releases/latest">Latest Release</a>
  </p>
</div>

## About The Project

[//]: # ([![Product Name Screen Shot][product-screenshot]]&#40;https://example.com&#41;)

This project aims to allow Growatt Inverters owners to take full ownership of their systems.
This can be for various reasons:

- Lack of internet connectivity
- Privacy concerns
- Data ownership

At the same time, not all users may want or be able to replace parts of their system to have it behave as they'd want
to.

Since the inverters by themselves do not have the capability to connect to the internet, they need a middleman to query
them and report the data online.
There are excellent projects that replace the middleman with a custom device that can interrogate the inverter(s) via
RS485, however it may not be the best solution, especially if the system is made of multiple inverters.

In this case, the other option is to intercept the messages of the middleman and act as a second one ourselves.
This way, the messages are decoded, and sent via MQTT or similar means (planned) to whatever system that may consume the
data.

Currently, the project allows for the messages exchanged between the system and Growatt's server to be forwarded by
doing as little modifications as possible.
It is planned to be able to disconnect the system from the internet for complete control.

It is known that there are different protocol versions used by Growatt devices.
Right now the project can "understand" **Protocol V6** (in particular, for SPH series inverters), however as soon as
data from other protocol versions will be available, the project will be updated to support them.

## Getting Started

### Please read this first

Right now the project is in a very early stage, and it is not ready for production use.
Still, it is possible to use it for non-critical systems and to test it.
I am using it on my system, and it is working fine; however, I cannot guarantee that it will work for you too.

I'd be glad to receive feedback, and to help you set it up if you need it.

Please consider the releases that I generate as the "stable" versions, and the dev branch as the "unstable" ones.
I am personally pushing the releases to my system, so usually, they don't have any main or breaking issues.

During the project lifetime, I'm planning on releasing only GNU/Linux executables and corresponding Docker images.

### Prerequisites

- A system running GNU/Linux (tested on Ubuntu 22.04) or Docker
- A compatible Growatt inverter (tested on two SPH10000 TL3 BH-UP with a Smart Energy Meter)
- TCP Port 5279 free on your system
- A Postgres database (optional in the future) with the TimescaleDB extension (included in the docker-compose file)

### Set-up

There will be some steps involved, which will be listed here:

1. Docker setup
2. Configuration

Unfortunately, right now the only "official" way to set up the server is to use Docker.
It helps a lot to reduce the number of variables that can cause issues, and it is easier to set up.

Let's start with the Docker setup.

### Docker setup

We will be using the docker image present on the [Docker Hub](https://hub.docker.com/r/stefa168/growatt_server) or
the [GitHub Container Registry](https://github.com/stefa168/growatt_server/pkgs/container/growatt_server).

Consider using the release marked as `latest` for the most stable version.

Then, prepare the docker-compose file. You can use the one present in this repository as a template.

In the docker-compose file you'll also find the configuration for a TimescaleDB database.
The server currently works only with PostgreSQL databases; Timescale is useful because the server produces a lot of
daily data.
Plus, buckets and time-series are a good fit for the data produced by the server.

### Configuration

Create a `.secrets` folder in the same directory as the docker-compose file. Here we'll put secrets that we don't want
to be saved directly in the compose file. You can use the `.postgres-password` file as an example. More
details [here](https://docs.docker.com/compose/use-secrets/).

Configure the username and password for the database as you like.

Then, create a `config.yaml` file in the same directory as the docker-compose file. You can use the `config.yaml` file
present in this repository as an example.

## Usage

Starting the server is simply a matter of running the executable.
By default, it looks for a configuration file in the same directory, however it can be changed with the `-c`
or `--config_path` optional parameter.

Please take a look at the default [configuration file for more information](config.yaml).

If a log level different from `INFO` is necessary, set the environment variable `LOG_LEVEL` to the required
level (`DEBUG`, `TRACE`, etc.)

The server will relay data to the endpoint specified in the configuration file.
It defaults to Growatt's servers on `server.growatt.com`.

For more command-line options, use the `--help` option.

## Roadmap

- [ ] Message interception
    - [ ] Proxy
        - [x] Basic proxy
        - [ ] Proxy with filtering features (for unwanted remote control)
    - [ ] Impersonator
    - Protocols
        - [x] Protocol v6
            - [x] SPH Inverters
            - [ ] Other Inverters
        - [ ] ?
- [ ] Data
    - [x] Storage
    - [ ] MQTT
    - [ ] Home Assistant
- [ ] Frontend

See the [open issues](https://github.com/stefa168/growatt_server/issues) for a full list of proposed features (and known
issues).

<!-- CONTRIBUTING -->

## Contributing

Contributions are vital to the Open Source ecosystem.
If you have any suggestion or improvement, please submit it!

You can open an issue, or fork the repository and then make a pull request with your new features and suggestions.

The commit messages are expected to follow
the [Conventional Commits format](https://www.conventionalcommits.org/en/v1.0.0/).

<!-- LICENSE -->

## License

Distributed under the GNU GPL3 License. See `LICENSE.md` for more information.

<!-- CONTACT -->

## Contact

Stefano Vittorio Porta - @stefa168 on Twitter and Telegram

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
