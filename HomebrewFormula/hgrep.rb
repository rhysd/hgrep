class Hgrep < Formula
  version '0.3.0'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 '2358d367511afade36e00b0c72ad5d73402350bd0b992edc21e367c78026c505' # x86_64-apple-darwin
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '698b31591abaa72f032f535863c1e5b30f9b2ce4cc778feee6f2bc48c334437e' # aarch64-apple-darwin
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '4a822e5522f26a72204362a888302fd81f3b5d6d7569bc3283d3e8761754dd93' # x86_64-unknown-linux-gnu
  end

  def install
    bin.install 'hgrep'
    hgrep = bin/'hgrep'
    # Check if hgrep exists to avoid #6
    if hgrep.exist? && hgrep.executable?
      output = Utils.safe_popen_read(hgrep, '--generate-completion-script', 'zsh')
      (zsh_completion/'_hgrep').write output
      output = Utils.safe_popen_read(hgrep, '--generate-completion-script', 'bash')
      (bash_completion/'hgrep').write output
      output = Utils.safe_popen_read(hgrep, '--generate-completion-script', 'fish')
      (fish_completion/'hgrep.fish').write output
      output = Utils.safe_popen_read(hgrep, '--generate-man-page')
      (man1/'hgrep.1').write output
    end
  end

  test do
    system "#{bin}/hgrep", '--version'
  end
end
