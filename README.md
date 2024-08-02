# NetCanv

A lightweight app for drawing with other people over the Internet.

![screenshot](contrib/screenshots.png)

<p align="center">
A screenshot of my imaginary friend and I scribbling things together in NetCanv.
</p>

- **Lightweight.** The whole app fits in less than 10 MB, and unlike web apps you only ever have to
  download a specific version once.
- **Fast.** NetCanv uses a GPU-accelerated canvas to deliver you the smoothest drawing experience
  possible. Additionally, NetCanv's network protocol is really simple, which makes it run well even
  on slow connections.
- **Free.** Being licensed under the Apache License 2.0, NetCanv is and will always be free to use
  and modify, by anyone, for any purpose.
- **Open.** NetCanv is an open platform - if you're tech-savvy, you're free to set up an alternative
  server, and if you're into programming we invite you to [contribute](https://github.com/liquidev/netcanv/pulls)!
- **Handmade with 💙** by people who care about good software. We take your feedback seriously -
  head over to [issues](https://github.com/liquidev/netcanv) and tell us what's bugging you!

## Compiling

Should be as simple as:

```sh
$ cargo build --release
# or, if you just want to run the app:
$ cargo run --release
```

**NOTE:** The "Open source licenses" icon will not show up in the lobby screen unless you have
cargo-about installed. To install it, use:

```sh
cargo install cargo-about
```

### Features

Alternate rendering backends can be chosen by passing in features via the `--features` flag.

- `renderer-opengl` (default) – The OpenGL rendering backend. May be incomplete or buggy in some
  places on certain drivers, please file issue reports if you find bugs!
- `renderer-wgpu` - The wgpu rendering backend. Has feature parity with OpenGL rendering backend,
  but is a bit buggy. Will replace OpenGL backend in the future.

For example, to build with the wgpu backend:

```
$ cargo build --no-default-features --features renderer-wgpu --release
```

Do note that PRs implementing alternate backends will not be merged, because the rendering API is
still in flux and may change at any time. More backends may be added after 1.0 is released.

#### Skia backend

There used to be a Skia backend, but it was removed because it was an unsupported, unnecessary
maintenance burden. The last tag to feature this backend is [0.5.0](https://github.com/liquidev/netcanv/tree/0.5.0).

### Relay

NetCanv will connect to the official relay, hosted at <https://netcanv.org>, by default. However, if
you want to host your own relay for whatever reason, it's quite simple to do.

To run the relay server, simply do:

```sh
cargo run -p netcanv-relay
```

This will allow you to host and join new rooms locally.

NetCanv's CI also provides builds of the relay for x86_64 and aarch64, so that you can set it up
on a VPS, a Raspberry Pi, or a regular ol' computer. The relay is very lightweight and doesn't
require much compute power - your main limit is Internet bandwidth.

#### Nginx

If you have nginx running on your server, you can create a reverse proxy to the relay by adding
this to your `server {}` block:

```nginx
location /relay {
   proxy_pass http://localhost:62137;
   proxy_http_version 1.1;
   proxy_set_header Upgrade $http_upgrade;
   proxy_set_header Connection "upgrade";
}
```

It's also highly recommended to use a tool like [certbot](https://certbot.eff.org/) to enable
support for encryption. NetCanv assumes that servers support encryption by default, by prepending
`wss://` to the URL in the relay server text field if another scheme isn't already present.

## "Tutorial"

<details><summary>NetCanv was originally made as part of a YouTube "tutorial" series.</summary>

The series is in Polish (!) and can be found on
[YouTube](https://www.youtube.com/playlist?list=PL1Hg-PZUNFkeRdErHKx3Z7IwhJNgij3bJ).

Individual episodes:

1. [Introduction](https://www.youtube.com/watch?v=ZeSXVgjrivY)
2. [Drawing and GUI](https://www.youtube.com/watch?v=MVEILFrPKnY)
3. [Refactoring and ∞](https://www.youtube.com/watch?v=mECVCb87sAQ)
4. Networking – coming soon (never)

Again, note that the tutorials are in Polish.

### Purpose

The main purpose of this tutorial series is to show how to build a desktop app
using Rust and Skia, together with peer-to-peer communication for realtime
collaboration.

I generally don't like explaining every small detail in my videos. I'd rather
showcase the cool and interesting parts about the development process. So don't
consider this as a general Rust application development tutorial – treat it more
like a devlog with some educational, comedic, and artistic value sprinkled
over it.

</details>
