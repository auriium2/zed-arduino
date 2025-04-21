# Arduino Language Support for Zed Editor

This plugin adds support for the [`arduino-language-server`](https://github.com/arduino/arduino-language-server) to the Zed Editor. It also enables syntax highlighting for `.ino` files.

## Usage

Due to my non-extensive knowledge of how to make Zed extensions, this plugin is not fully plug and play.

While it will download the `arduino-language-server` for you, you will need the [`arduino-cli`](https://github.com/arduino/arduino-cli) installed and [`clangd`](https://github.com/clangd/clangd) installed and available to the Zed editor.

Furthermore, you will need to specify your board's FQBN (Fully qualified board name). Example:

```jsonc
// .zed/settings.json
{
  // associate .ino with c++
  "file_types": {
    "Arduino": ["cpp", "h", "hpp"],
  },

  "lsp": {
    "arduino-language-server": {
      "binary": {
        "arguments": [
          "-fqbn",
          "esp32:esp32:esp32s3:CDCOnBoot=cdc,CPUFreq=240,DFUOnBoot=default,FlashMode=qio,FlashSize=16M,MSCOnBoot=default,PSRAM=opi,PartitionScheme=app3M_fat9M_16MB,USBMode=hwcdc",
        ],
      },
    },
  },
}
```

## Installation

Due to the hackyness of this plugin, I've not published it to the Zed plugin repository. To install you will need rustup or the rust toolchain installed. I've provided a devenv environment if you have devenv installed.

You can then manually install the plugin on the Zed extensions page by clicking the "Install Dev Extension" plugin and pointing it to this directory.

## PRs and issues

Open to improvements! If you think there's a better way to handle finding the FQBN, let me know!
