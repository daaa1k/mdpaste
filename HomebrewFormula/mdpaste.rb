class Mdpaste < Formula
  desc "Paste clipboard image as Markdown link"
  homepage "https://github.com/daaa1k/mdpaste"
  license "MIT"
  version "0.7.1"

  on_macos do
    on_arm do
      url "https://github.com/daaa1k/mdpaste/releases/download/v#{version}/mdpaste-macos-aarch64"
      sha256 "c55c7f7356c33ce4c421ed7214e79c7be1e49bfcb3aff00fdac94f3dfc8c2b80" # macos-aarch64
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/daaa1k/mdpaste/releases/download/v#{version}/mdpaste-linux-x86_64"
      sha256 "f9b62820fc1d3ed16d2a6873b3e5207f155bcd7dd61ce3d080942ade7d05b9b4" # linux-x86_64
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
