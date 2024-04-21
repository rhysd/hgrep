class Hgrep < Formula
  version '0.3.7'
  desc 'hgrep is grep with human-friendly search output'
  homepage 'https://github.com/rhysd/hgrep'

  if OS.mac?
    if Hardware::CPU.intel?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-apple-darwin.zip"
      sha256 '31d94049db7b11fc713ac5e4d0a9db9ebc5093f9b60582755b0d4616aa11df89' # x86_64-apple-darwin
    end
    if Hardware::CPU.arm?
      url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-aarch64-apple-darwin.zip"
      sha256 '3522f116f4ed1b8dcf02f079409b55a1f5a66cebda2baf8934907a186ef6986b' # aarch64-apple-darwin
    end
  elsif OS.linux?
    url "https://github.com/rhysd/hgrep/releases/download/v#{version}/hgrep-v#{version}-x86_64-unknown-linux-gnu.zip"
    sha256 '3eeeeeb220e3ede6d433a3ab31c56167e4edb3e936bee722833595297f5e432c' # x86_64-unknown-linux-gnu
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
