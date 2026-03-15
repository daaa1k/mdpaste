class Mdpaste < Formula
  desc "Paste clipboard image as Markdown link"
  homepage "https://github.com/daaa1k/mdpaste"
  license "MIT"
  version "0.8.0"

  on_macos do
    on_arm do
      url "https://github.com/daaa1k/mdpaste/releases/download/v#{version}/mdpaste-macos-aarch64"
      sha256 "70e2e9e7c0490257f0ac361c12ad23fe5c7bfd5b572b8b7f140bf82ae67fa146" # macos-aarch64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/daaa1k/mdpaste/releases/download/v#{version}/mdpaste-linux-x86_64"
      sha256 "7dffe5ac13bdbfb5e62bbacdfb893d0930b9ddd38b59da26d5a827793b3bd619" # linux-x86_64
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
