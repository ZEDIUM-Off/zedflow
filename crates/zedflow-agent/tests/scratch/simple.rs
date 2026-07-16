const CANONICAL_SAMPLE: &str = "references/pi/packages/agent/test/scratch/simple.ts";
const LIVE_PROVIDER_BLOCKER: &str = "requires OpenAI or Cloudflare AI Gateway credentials, network access, local user/project .pi skills and prompts, and may trigger provider OAuth/browser login; AT7 must not require credentials or browser sessions";

#[test]
#[ignore = "live scratch sample skipped: requires OpenAI/Cloudflare credentials, network access, local .pi resource dirs, and possible OAuth/browser login"]
fn scratch_simple_live_provider_sample_is_represented_but_not_run() {
    assert_eq!(
        CANONICAL_SAMPLE,
        "references/pi/packages/agent/test/scratch/simple.ts"
    );
    assert!(LIVE_PROVIDER_BLOCKER.contains("must not require credentials"));
}
