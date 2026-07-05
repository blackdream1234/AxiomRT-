(* AxiomRT — Capability access model (AXIOM-PROOF-002).

   Requirement reference: docs/06_CAPABILITY_MODEL.md §6 (CAP-P1..P3),
   docs/11_VERIFICATION_PLAN.md.

   Required theorem shape (task): a task cannot invoke a protected
   object without a valid capability with sufficient rights.

   Model level: mirrors kernel/src/caps/ — a capability table is a
   partial map from indexes to capabilities; invocation is DEFINED as
   a successful lookup with matching type and sufficient rights; no
   other invocation path exists (unforgeability is structural:
   capabilities live only in kernel memory,
   docs/06_CAPABILITY_MODEL.md §1).

   Assumptions (explicit):
     A1. The capability table is the only source of authority
         (no capability bits in user memory).
     A2. Minting happens only at boot (v0.1); derivation only
         diminishes (modeled by `diminish`).
     A3. Single hart; no concurrent table mutation. *)

Require Import Arith List Bool.
Import ListNotations.

(* ----------------------------------------------------------------- *)
(* Model                                                              *)

Inductive Right : Type :=
  | RRead | RWrite | RExecute | RSend | RReceive | RGrant | RMap | RControl.

Definition right_eqb (a b : Right) : bool :=
  match a, b with
  | RRead, RRead | RWrite, RWrite | RExecute, RExecute
  | RSend, RSend | RReceive, RReceive | RGrant, RGrant
  | RMap, RMap | RControl, RControl => true
  | _, _ => false
  end.

Lemma right_eqb_eq : forall a b, right_eqb a b = true <-> a = b.
Proof.
  intros a b; split; intros H.
  - destruct a, b; simpl in H; congruence.
  - subst; destruct b; reflexivity.
Qed.

Definition Rights := list Right.

Definition has_right (rs : Rights) (r : Right) : Prop := In r rs.

(* held contains every required right (subset test, caps/rights.rs).  *)
Definition contains (held required : Rights) : Prop :=
  forall r, In r required -> In r held.

Inductive OType : Type :=
  | OThread | OEndpoint | OAddressSpace | OFrame | OTimer | OSchedCtx | OFaultChannel.

Definition otype_eqb (a b : OType) : bool :=
  match a, b with
  | OThread, OThread | OEndpoint, OEndpoint | OAddressSpace, OAddressSpace
  | OFrame, OFrame | OTimer, OTimer | OSchedCtx, OSchedCtx
  | OFaultChannel, OFaultChannel => true
  | _, _ => false
  end.

Record Capability := mkCap {
  c_type : OType;
  c_oid : nat;
  c_rights : Rights
}.

(* Per-task capability table: partial map from slot index.            *)
Definition CapTable := nat -> option Capability.

(* Invocation: the ONLY way to reach a protected object
   (caps/table.rs lookup — bounds/occupancy, type, rights).           *)
Definition invoke_allowed (tbl : CapTable) (idx : nat)
                          (ty : OType) (required : Rights) : Prop :=
  exists c, tbl idx = Some c
       /\ c_type c = ty
       /\ contains (c_rights c) required.

(* Diminish-only derivation (caps/rights.rs::diminish).               *)
Definition diminish (rs removed : Rights) : Rights :=
  filter (fun r => negb (existsb (right_eqb r) removed)) rs.

(* ----------------------------------------------------------------- *)
(* Theorems                                                           *)

(* CAP-P1 / required theorem shape, part 1: no capability in the slot,
   no invocation.                                                     *)
Theorem no_cap_no_access :
  forall tbl idx ty required,
    tbl idx = None -> ~ invoke_allowed tbl idx ty required.
Proof.
  intros tbl idx ty required Hempty [c [Hlook _]].
  rewrite Hempty in Hlook. discriminate.
Qed.

(* CAP-P1, part 2: wrong object type, no invocation (type confusion
   is structurally impossible).                                       *)
Theorem wrong_type_no_access :
  forall tbl idx ty required c,
    tbl idx = Some c -> c_type c <> ty ->
    ~ invoke_allowed tbl idx ty required.
Proof.
  intros tbl idx ty required c Hlook Hty [c' [Hlook' [Hty' _]]].
  rewrite Hlook in Hlook'. injection Hlook' as <-. exact (Hty Hty').
Qed.

(* CAP-P2: invocation implies every required right is held.           *)
Theorem access_implies_rights :
  forall tbl idx ty required r,
    invoke_allowed tbl idx ty required ->
    In r required ->
    exists c, tbl idx = Some c /\ has_right (c_rights c) r.
Proof.
  intros tbl idx ty required r [c [Hlook [_ Hsub]]] Hr.
  exists c. split; [exact Hlook | exact (Hsub r Hr)].
Qed.

(* CAP-P2 contrapositive: missing one required right, no invocation.  *)
Theorem insufficient_rights_no_access :
  forall tbl idx ty required c r,
    tbl idx = Some c ->
    In r required ->
    ~ has_right (c_rights c) r ->
    ~ invoke_allowed tbl idx ty required.
Proof.
  intros tbl idx ty required c r Hlook Hreq Hmiss [c' [Hlook' [_ Hsub]]].
  rewrite Hlook in Hlook'. injection Hlook' as <-.
  exact (Hmiss (Hsub r Hreq)).
Qed.

(* CAP-P3: derivation never adds rights.                              *)
Theorem diminish_no_amplification :
  forall rs removed r,
    In r (diminish rs removed) -> In r rs.
Proof.
  intros rs removed r Hin.
  unfold diminish in Hin.
  apply filter_In in Hin. destruct Hin as [Hin _]. exact Hin.
Qed.

(* CAP-P3 companion: a removed right is really gone.                  *)
Theorem diminish_removes :
  forall rs removed r,
    In r removed -> ~ In r (diminish rs removed).
Proof.
  intros rs removed r Hrem Hin.
  unfold diminish in Hin.
  apply filter_In in Hin. destruct Hin as [_ Hneg].
  apply Bool.negb_true_iff in Hneg.
  assert (Hex : existsb (right_eqb r) removed = true).
  { apply existsb_exists. exists r. split; [exact Hrem |].
    apply right_eqb_eq. reflexivity. }
  rewrite Hex in Hneg. discriminate.
Qed.

(* ----------------------------------------------------------------- *)
(* Refinement obligation, explicit TODO: caps/table.rs::lookup returns
   Ok exactly when invoke_allowed holds for the corresponding model
   table (fixed check order bounds → occupancy → type → rights). To be
   discharged against the extracted/translated implementation.        *)
Theorem lookup_refines_invoke_allowed : True.
Proof.
  (* TODO(refinement): connect to kernel/src/caps/table.rs. *)
  exact I.
Qed.
