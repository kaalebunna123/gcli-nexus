### Changes
- Centralized Cloud Code assist endpoints (`loadCodeAssist`, `onboardUser`) in `config.rs` to eliminate hard-coded URLs scattered across the OAuth flow.
- Fixed the Gemini Code Assist onboarding call to use the correct `v1internal:onboardUser` endpoint, preventing provisioning failures.
