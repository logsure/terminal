class Logsure < Formula
  desc "Terminal console for LogSure: analytics, the inbox and the waitlists"
  homepage "https://github.com/logsure/terminal"
  url "https://github.com/logsure/terminal.git",
      tag:      "v0.2.0",
      revision: "36985a818448846b3c768c58b69669410177e2f6"
  license "MPL-2.0"
  head "https://github.com/logsure/terminal.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/logsure --version")
    ENV["LOGSURE_CONFIG"] = testpath/"config.toml"
    assert_match "No servers yet", shell_output("#{bin}/logsure ctx")
  end
end
