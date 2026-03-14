class Mdpaste < Formula
  desc "Paste clipboard image as Markdown link"
  homepage "https://github.com/daaa1k/mdpaste"
  license "MIT"
  version "0.7.0"

  on_macos do
    on_arm do
      url "https://github.com/daaa1k/mdpaste/releases/download/v#{version}/mdpaste-macos-aarch64"
      sha256 "4111748685729c95f8f856fa1a74fce024f713fa33bc759fd9613e89a71d6388" # macos-aarch64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/daaa1k/mdpaste/releases/download/v#{version}/mdpaste-linux-x86_64"
      sha256 "0b3e4d4dfc5576b20a7a836836a576705492371509d077bb61d4b834db7fe07d" # linux-x86_64
    end
  end

  def install
    if OS.mac?
      bin.install "mdpaste-macos-aarch64" => "mdpaste"
    else
      bin.install "mdpaste-linux-x86_64" => "mdpaste"
    end
  end

  test do
    system bin/"mdpaste", "--help"
  end
end
