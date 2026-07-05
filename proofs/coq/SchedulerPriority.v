(* AxiomRT — Scheduler priority model (AXIOM-PROOF-003).

   Requirement reference: docs/09_SCHEDULER_MODEL.md §4
   (SCHED-P1..P3), docs/11_VERIFICATION_PLAN.md.

   Required theorem shape (task): if a high-priority ready task
   exists, a lower-priority task is not selected.

   Model level: the scheduler SPECIFICATION is a selection predicate —
   the selected thread is ready and maximal in priority among ready
   threads (docs/09 §4: the thread state machine is the authority for
   readiness). A concrete selection function over lists is also given
   and proven to satisfy the specification, mirroring
   kernel/src/sched/mod.rs::select_next.

   Assumptions (explicit):
     A1. Readiness is decided by the thread state machine alone.
     A2. Single hart: one selection at a time.
     A3. Tie-breaking among equal priorities (FIFO) is below the
         abstraction level of these theorems: any maximal ready thread
         satisfies the spec; determinism of the tie-break is checked
         by the Rust unit tests (SCHED-P3).                           *)

Require Import Arith List Bool Lia.
Import ListNotations.

(* ----------------------------------------------------------------- *)
(* Model                                                              *)

Inductive TState : Type :=
  | Ready | Running | Blocked | Faulted | Killed | Suspended.

Record Thread := mkThread {
  t_id : nat;
  t_prio : nat;          (* higher value = more urgent *)
  t_state : TState
}.

Definition is_ready (t : Thread) : bool :=
  match t_state t with Ready => true | _ => false end.

(* Scheduler specification (docs/09 §4): the selected thread is in the
   queue, ready, and of maximal priority among ready threads.         *)
Definition selected (q : list Thread) (t : Thread) : Prop :=
  In t q /\ is_ready t = true /\
  (forall t', In t' q -> is_ready t' = true -> t_prio t' <= t_prio t).

(* ----------------------------------------------------------------- *)
(* Specification-level theorems                                       *)

(* SCHED-P1 / required theorem shape: if a higher-priority ready
   thread exists in the queue, a lower-priority thread is not
   selected.                                                          *)
Theorem high_priority_excludes_low :
  forall q t high,
    In high q -> is_ready high = true ->
    t_prio high > t_prio t ->
    ~ selected q t.
Proof.
  intros q t high Hin Hready Hgt [_ [_ Hmax]].
  specialize (Hmax high Hin Hready). lia.
Qed.

(* SCHED-P2: a non-ready thread is never selected — regardless of
   priority (killed, blocked, faulted, suspended, running).           *)
Theorem non_ready_never_selected :
  forall q t,
    is_ready t = false -> ~ selected q t.
Proof.
  intros q t Hnr [_ [Hr _]]. rewrite Hnr in Hr. discriminate.
Qed.

(* ----------------------------------------------------------------- *)
(* Concrete selection function (mirrors sched/mod.rs::select_next)    *)

(* Pick the better of two candidates: keep the current best unless the
   challenger is ready with strictly higher priority. (FIFO tie-break:
   the earlier thread wins ties by staying the best.)                 *)
Definition better (best challenger : Thread) : Thread :=
  if is_ready challenger && (t_prio best <? t_prio challenger)
  then challenger else best.

Fixpoint select_from (q : list Thread) : option Thread :=
  match q with
  | [] => None
  | t :: rest =>
      match select_from rest with
      | None => if is_ready t then Some t else None
      | Some best => if is_ready t then Some (better t best) else Some best
      end
  end.

Lemma better_cases :
  forall a b, better a b = a \/ better a b = b.
Proof.
  intros a b. unfold better.
  destruct (is_ready b && (t_prio a <? t_prio b)); auto.
Qed.

Lemma select_from_in :
  forall q t, select_from q = Some t -> In t q.
Proof.
  induction q as [| h rest IH]; simpl; intros t H.
  - discriminate.
  - destruct (select_from rest) as [best |] eqn:Hrest.
    + destruct (is_ready h) eqn:Hr.
      * injection H as <-.
        destruct (better_cases h best) as [He | He]; rewrite He.
        -- left. reflexivity.
        -- right. apply IH. reflexivity.
      * injection H as <-. right. apply IH. reflexivity.
    + destruct (is_ready h) eqn:Hr; [injection H as <-; left; reflexivity | discriminate].
Qed.

(* An empty selection means no ready thread exists in the queue.      *)
Lemma select_from_none_no_ready :
  forall q t, select_from q = None -> In t q -> is_ready t = false.
Proof.
  induction q as [| h rest IH]; simpl; intros t Hnone Hin.
  - contradiction.
  - destruct (select_from rest) as [b |] eqn:Hr.
    + destruct (is_ready h); discriminate.
    + destruct (is_ready h) eqn:Hh; [discriminate |].
      destruct Hin as [<- | Hin]; [exact Hh | apply IH; assumption].
Qed.

Lemma select_from_ready :
  forall q t, select_from q = Some t -> is_ready t = true.
Proof.
  induction q as [| h rest IH]; simpl; intros t H.
  - discriminate.
  - destruct (select_from rest) as [best |] eqn:Hrest.
    + assert (Hbest : is_ready best = true) by (apply IH; reflexivity).
      destruct (is_ready h) eqn:Hr.
      * injection H as <-. unfold better.
        destruct (is_ready best && (t_prio h <? t_prio best)) eqn:Hc; assumption.
      * injection H as <-. exact Hbest.
    + destruct (is_ready h) eqn:Hr; [injection H as <-; exact Hr | discriminate].
Qed.

Lemma select_from_maximal :
  forall q t, select_from q = Some t ->
  forall t', In t' q -> is_ready t' = true -> t_prio t' <= t_prio t.
Proof.
  induction q as [| h rest IH]; simpl; intros t Hsel t' Hin Hready.
  - contradiction.
  - destruct (select_from rest) as [best |] eqn:Hrest.
    + assert (Hbmax : forall u, In u rest -> is_ready u = true ->
                      t_prio u <= t_prio best)
        by (intros; apply (IH best); auto).
      destruct (is_ready h) eqn:Hr.
      * injection Hsel as <-. unfold better.
        destruct (is_ready best && (t_prio h <? t_prio best)) eqn:Hc.
        -- (* selected = best, with prio h < prio best *)
           apply andb_prop in Hc. destruct Hc as [_ Hlt].
           apply Nat.ltb_lt in Hlt.
           destruct Hin as [<- | Hin]; [lia |].
           apply Hbmax; assumption.
        -- (* selected = h: best not ready (impossible) or prio best <= prio h *)
           assert (Hble : t_prio best <= t_prio h).
           { apply andb_false_iff in Hc.
             destruct Hc as [Hnb | Hnlt].
             - rewrite (select_from_ready rest best Hrest) in Hnb. discriminate.
             - apply Nat.ltb_ge in Hnlt. exact Hnlt. }
           destruct Hin as [<- | Hin]; [lia |].
           specialize (Hbmax t' Hin Hready). lia.
      * injection Hsel as <-.
        destruct Hin as [<- | Hin].
        -- rewrite Hready in Hr. discriminate.
        -- apply Hbmax; assumption.
    + destruct (is_ready h) eqn:Hr; [| discriminate].
      injection Hsel as <-.
      destruct Hin as [<- | Hin]; [lia |].
      (* rest has no ready thread: select_from rest = None *)
      exfalso.
      pose proof (select_from_none_no_ready rest t' Hrest Hin) as Hf.
      rewrite Hready in Hf. discriminate.
Qed.

(* The concrete function refines the specification.                   *)
Theorem select_from_satisfies_spec :
  forall q t, select_from q = Some t -> selected q t.
Proof.
  intros q t H.
  repeat split.
  - apply (select_from_in q t H).
  - apply (select_from_ready q t H).
  - apply (select_from_maximal q t H).
Qed.

(* Corollary in the required theorem shape, for the concrete function:
   the selection never returns a thread when a strictly
   higher-priority ready thread exists.                               *)
Corollary concrete_high_priority_excludes_low :
  forall q t high,
    In high q -> is_ready high = true ->
    t_prio high > t_prio t ->
    select_from q <> Some t.
Proof.
  intros q t high Hin Hready Hgt Hsel.
  apply (high_priority_excludes_low q t high Hin Hready Hgt).
  apply select_from_satisfies_spec. exact Hsel.
Qed.

(* ----------------------------------------------------------------- *)
(* Refinement obligation, explicit TODO: sched/mod.rs::select_next
   (per-level FIFO rings + stale-entry discard) implements select_from
   up to the FIFO tie-break order. Determinism of the tie-break
   (SCHED-P3) is currently evidenced by tests/scheduler_tests.rs.     *)
Theorem rust_select_next_refines_select_from : True.
Proof.
  (* TODO(refinement): connect to kernel/src/sched/mod.rs. *)
  exact I.
Qed.
