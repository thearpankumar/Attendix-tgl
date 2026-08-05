-- Move WebAuthn from hand-rolled parsing to the `webauthn-rs` state machine.
--
-- The previous implementation never verified an assertion signature and never
-- verified an attestation: it decoded `authenticatorData`, read the UV flag and
-- the counter, and accepted the credential. Every credential enrolled under it
-- could have been created by anyone for any roll number, and every assertion
-- against it could be forged with a garbage signature. Those rows carry no
-- security value, and the raw COSE key stored in `public_key` is not the state
-- `webauthn-rs` replays at finish time. Every student re-enrolls.
DELETE FROM webauthn_credentials;

-- `passkey` holds a serialised `webauthn_rs::prelude::Passkey` (JSON), which
-- carries the public key, the credential ID, the counter and the backup/UV
-- policy that verification needs.
ALTER TABLE webauthn_credentials DROP COLUMN public_key;
ALTER TABLE webauthn_credentials ADD COLUMN passkey BYTEA NOT NULL;

-- WebAuthn's user handle must be an opaque, stable identifier. It is derived
-- deterministically from the roll number (UUIDv5) so discoverable-credential
-- ("conditional UI") logins can map an assertion back to a student without the
-- roll number ever being guessable from the handle alone.
ALTER TABLE webauthn_credentials ADD COLUMN user_handle UUID;
CREATE INDEX idx_webauthn_credentials_user_handle ON webauthn_credentials (user_handle);

-- A ceremony's server-side state (challenge, user-verification policy, allowed
-- credentials) must survive between start and finish. Storing it is what makes
-- the challenge binding real: finish replays *this* state rather than trusting
-- a challenge string echoed back by the client.
ALTER TABLE webauthn_challenges ADD COLUMN state JSONB;

-- In-flight ceremonies from the old scheme have no state to replay.
DELETE FROM webauthn_challenges;
