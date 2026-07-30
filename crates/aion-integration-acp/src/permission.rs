//! The permission policy an [`AcpHarness`] must be constructed with — how this adapter answers
//! `session/request_permission`.
//!
//! # Why the policy is required, and why there is no default
//!
//! `session/request_permission` is a **request**: the agent blocks on the answer. An unanswered
//! request wedges the run until the activity times out, with no transcript explanation. An Aion
//! activity is unattended by default — there is no operator sitting in front of it to click
//! *Allow* — so there must ALWAYS be an answer, and the answer has to be decided before the
//! request arrives.
//!
//! [`PermissionPolicy`] therefore has **no [`Default`] impl** and is a required argument of
//! [`AcpHarness::new`]. A default here would be an assumed default on a security decision: either
//! it silently approves an agent's file writes and shell commands, or it silently blocks work an
//! operator meant to allow. Both are the kind of quiet, load-bearing guess this workspace forbids.
//! The composition root states the policy out loud.
//!
//! # Auditability
//!
//! Whatever the policy decides, [`crate::pump`] emits the request AND the decision onto the
//! [`aion_core::ActivityEvent`] transcript before the answer is written to the wire. A silent
//! auto-approval is exactly the thing an operator must be able to reconstruct after the fact.
//!
//! [`AcpHarness`]: crate::AcpHarness
//! [`AcpHarness::new`]: crate::AcpHarness::new
//!
//! [`PermissionPolicy`]: crate::permission::PermissionPolicy

use agent_client_protocol_schema::v1::{
    PermissionOption, PermissionOptionKind, RequestPermissionOutcome, SelectedPermissionOutcome,
};

/// How this adapter answers an agent's `session/request_permission`.
///
/// The agent offers a list of [`PermissionOption`]s and the policy picks one — or answers
/// [`RequestPermissionOutcome::Cancelled`] when the offered list contains nothing the policy is
/// willing to select. Every arm always produces an answer; none can leave the agent blocked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionPolicy {
    /// Refuse every request: select the first `reject_once` or `reject_always` option offered, in
    /// the order the agent listed them.
    ///
    /// When the agent offered no reject option at all, the answer is
    /// [`RequestPermissionOutcome::Cancelled`] — the only way to decline without selecting an
    /// approval the policy forbids.
    ///
    /// This is the policy for an activity that must not let an agent take a gated action:
    /// read-only analysis, an untrusted prompt, a dry run.
    Deny,
    /// Approve one-shot requests: select the first `allow_once` option offered.
    ///
    /// It deliberately never falls back to `allow_always`. "Remember this choice" is a durable
    /// grant that outlives the activity, and no unattended activity has the standing to make one;
    /// an agent that offers only `allow_always` gets
    /// [`RequestPermissionOutcome::Cancelled`] rather than a widened approval. It likewise never
    /// falls back to a reject option — that would report an approval policy as a denial.
    ///
    /// This is the policy for an activity whose whole purpose is to let the agent act (edit files,
    /// run commands) inside a workspace the operator has already scoped for it.
    AllowOnce,
}

impl PermissionPolicy {
    /// A stable, human-readable label for the policy, recorded on the transcript alongside the
    /// decision so an audit shows which rule produced it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Deny => "deny",
            Self::AllowOnce => "allow_once",
        }
    }

    /// Decides the outcome for one `session/request_permission`.
    ///
    /// Never fails and never abstains: every input — including an empty option list, which a
    /// non-conforming agent can send — yields an outcome that can be written straight back as the
    /// response.
    #[must_use]
    pub fn decide(self, options: &[PermissionOption]) -> RequestPermissionOutcome {
        let selected = match self {
            Self::Deny => first_of(
                options,
                &[
                    PermissionOptionKind::RejectOnce,
                    PermissionOptionKind::RejectAlways,
                ],
            ),
            Self::AllowOnce => first_of(options, &[PermissionOptionKind::AllowOnce]),
        };
        match selected {
            Some(option) => RequestPermissionOutcome::Selected(SelectedPermissionOutcome::new(
                option.option_id.clone(),
            )),
            // No acceptable option on offer. Cancelling is the protocol's own "no answer chosen"
            // outcome and is the honest report: the policy declined to pick from what was offered.
            None => RequestPermissionOutcome::Cancelled,
        }
    }
}

/// The first option whose kind appears in `wanted`, scanning `wanted` in preference order.
///
/// Preference order is over the KINDS, not over the agent's list: `Deny` prefers a `reject_once`
/// anywhere in the list over a `reject_always` that happens to appear earlier, because the
/// narrower grant is always the safer selection. Within one kind, the agent's own ordering wins.
fn first_of<'a>(
    options: &'a [PermissionOption],
    wanted: &[PermissionOptionKind],
) -> Option<&'a PermissionOption> {
    wanted
        .iter()
        .find_map(|kind| options.iter().find(|option| option.kind == *kind))
}

#[cfg(test)]
mod tests {
    use agent_client_protocol_schema::v1::{
        PermissionOption, PermissionOptionKind, RequestPermissionOutcome,
    };

    use super::PermissionPolicy;

    /// The option list a well-behaved agent offers for a gated tool call.
    fn full_options() -> Vec<PermissionOption> {
        vec![
            PermissionOption::new("a1", "Allow once", PermissionOptionKind::AllowOnce),
            PermissionOption::new("a2", "Always allow", PermissionOptionKind::AllowAlways),
            PermissionOption::new("r1", "Reject", PermissionOptionKind::RejectOnce),
            PermissionOption::new("r2", "Always reject", PermissionOptionKind::RejectAlways),
        ]
    }

    /// The `optionId` of a `selected` outcome, or `None` for `cancelled`.
    fn selected_id(outcome: &RequestPermissionOutcome) -> Option<String> {
        match outcome {
            RequestPermissionOutcome::Selected(selected) => Some(selected.option_id.to_string()),
            _ => None,
        }
    }

    #[test]
    fn deny_selects_the_reject_once_option() {
        let outcome = PermissionPolicy::Deny.decide(&full_options());
        assert_eq!(selected_id(&outcome).as_deref(), Some("r1"));
    }

    #[test]
    fn deny_falls_back_to_reject_always_when_no_reject_once_is_offered() {
        let options = vec![
            PermissionOption::new("a1", "Allow once", PermissionOptionKind::AllowOnce),
            PermissionOption::new("r2", "Always reject", PermissionOptionKind::RejectAlways),
        ];
        let outcome = PermissionPolicy::Deny.decide(&options);
        assert_eq!(selected_id(&outcome).as_deref(), Some("r2"));
    }

    #[test]
    fn deny_cancels_when_no_reject_option_is_offered() {
        // An agent that offers only approvals must not coerce an approval out of a deny policy.
        let options = vec![
            PermissionOption::new("a1", "Allow once", PermissionOptionKind::AllowOnce),
            PermissionOption::new("a2", "Always allow", PermissionOptionKind::AllowAlways),
        ];
        let outcome = PermissionPolicy::Deny.decide(&options);
        assert_eq!(outcome, RequestPermissionOutcome::Cancelled);
    }

    #[test]
    fn allow_once_selects_the_allow_once_option() {
        let outcome = PermissionPolicy::AllowOnce.decide(&full_options());
        assert_eq!(selected_id(&outcome).as_deref(), Some("a1"));
    }

    #[test]
    fn allow_once_never_widens_to_allow_always() {
        // "Remember this choice" outlives the activity; an unattended run cannot grant it.
        let options = vec![PermissionOption::new(
            "a2",
            "Always allow",
            PermissionOptionKind::AllowAlways,
        )];
        let outcome = PermissionPolicy::AllowOnce.decide(&options);
        assert_eq!(outcome, RequestPermissionOutcome::Cancelled);
    }

    #[test]
    fn an_empty_option_list_still_produces_an_answer() {
        // A non-conforming agent can send `options: []`. Both policies must still answer, because
        // silence wedges the run.
        for policy in [PermissionPolicy::Deny, PermissionPolicy::AllowOnce] {
            assert_eq!(policy.decide(&[]), RequestPermissionOutcome::Cancelled);
        }
    }

    #[test]
    fn each_policy_has_a_distinct_audit_label() {
        assert_eq!(PermissionPolicy::Deny.label(), "deny");
        assert_eq!(PermissionPolicy::AllowOnce.label(), "allow_once");
    }
}
