class Logsure < Formula
  desc "Terminal console for LogSure: analytics, the inbox and the waitlists"
  homepage "https://github.com/logsure/terminal"
  url "https://github.com/logsure/terminal.git",
      tag:      "v0.5.0",
      revision: "0ae9786313d2ce6d86dc868841a4c69967e2d19f"
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
