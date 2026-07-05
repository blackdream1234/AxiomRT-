(* AxiomRT — Memory isolation model (AXIOM-PROOF-001).

   Requirement reference: docs/05_MEMORY_MODEL.md §10 (MEM-P1..P3),
   docs/11_VERIFICATION_PLAN.md.

   Required theorem shape (task): a task cannot read an address that is
   not mapped in its address space.

   Model level: this file models the *specification* of Phase 4
   (kernel/src/memory/): an address space is a partial map from virtual
   pages to (frame, permissions). Reads are defined by the model —
   there is no way to read outside it. The refinement obligation that
   the hardware page tables realize exactly this map is stated at the
   end and left as an explicit TODO.

   Assumptions (explicit, docs/11_VERIFICATION_PLAN.md §3):
     A1. One address space per task (v0.1).
     A2. The MMU enforces exactly the mappings of the model
         (refinement TODO below).
     A3. No shared frames in v0.1 (no_sharing hypothesis = the frame
         ownership invariant MEM-P4 established by the frame model).
     A4. Single hart; no concurrent mutation of address spaces. *)

Require Import Arith List Bool.
Import ListNotations.

(* ----------------------------------------------------------------- *)
(* Model                                                              *)

Definition TaskId := nat.
Definition VirtPage := nat.
Definition Frame := nat.

Record Perms := mkPerms {
  p_read : bool;
  p_write : bool
}.

(* An address space: partial map from virtual pages to frame+perms.   *)
Definition AddressSpace := VirtPage -> option (Frame * Perms).

(* The system: one address space per task (assumption A1).            *)
Definition System := TaskId -> AddressSpace.

Definition mapped (s : System) (t : TaskId) (v : VirtPage) : Prop :=
  exists fp, s t v = Some fp.

(* A read succeeds iff the page is mapped with read permission.
   This is the TOTAL definition of reading: no other read path exists
   (docs/05 §1: anything not explicitly mapped is inaccessible).      *)
Definition can_read (s : System) (t : TaskId) (v : VirtPage) : Prop :=
  exists f p, s t v = Some (f, p) /\ p_read p = true.

Definition can_write (s : System) (t : TaskId) (v : VirtPage) : Prop :=
  exists f p, s t v = Some (f, p) /\ p_write p = true.

(* Frame ownership uniqueness: no frame appears in two address spaces
   (v0.1 no-sharing form of MEM-P2/P4; established operationally by
   kernel/src/memory/frame.rs + pagetable.rs).                        *)
Definition no_sharing (s : System) : Prop :=
  forall t1 t2 v1 v2 f p1 p2,
    t1 <> t2 ->
    s t1 v1 = Some (f, p1) ->
    s t2 v2 = Some (f, p2) ->
    False.

(* ----------------------------------------------------------------- *)
(* Theorems                                                           *)

(* MEM-P3 / required theorem shape: a task cannot read an address that
   is not mapped in its address space.                                *)
Theorem no_unmapped_read :
  forall (s : System) (t : TaskId) (v : VirtPage),
    ~ mapped s t v -> ~ can_read s t v.
Proof.
  intros s t v Hnm [f [p [Heq _]]].
  apply Hnm. exists (f, p). exact Heq.
Qed.

(* Same for writes: no unmapped access of any kind.                   *)
Theorem no_unmapped_write :
  forall (s : System) (t : TaskId) (v : VirtPage),
    ~ mapped s t v -> ~ can_write s t v.
Proof.
  intros s t v Hnm [f [p [Heq _]]].
  apply Hnm. exists (f, p). exact Heq.
Qed.

(* MEM-P2 (v0.1 no-sharing form): under frame-ownership uniqueness,
   two distinct tasks can never reach the same frame.                 *)
Theorem task_isolation :
  forall (s : System) (t1 t2 : TaskId) (v1 v2 : VirtPage) (f : Frame)
         (p1 p2 : Perms),
    no_sharing s ->
    s t1 v1 = Some (f, p1) ->
    s t2 v2 = Some (f, p2) ->
    t1 = t2.
Proof.
  intros s t1 t2 v1 v2 f p1 p2 Hns H1 H2.
  destruct (Nat.eq_dec t1 t2) as [Heq | Hne].
  - exact Heq.
  - exfalso. exact (Hns t1 t2 v1 v2 f p1 p2 Hne H1 H2).
Qed.

(* ----------------------------------------------------------------- *)
(* Refinement obligation (assumption A2), explicit TODO:
   the hardware Sv39 page table produced by the kernel mapping
   function realizes exactly the model map (every hardware translation
   corresponds to a model mapping and vice versa). This connects the
   theorems above to the running kernel and can only be discharged
   against the MMU-activation implementation (post-v0.1 Phase 4/7
   integration).                                                      *)
Theorem pagetable_refines_model : True.
Proof.
  (* TODO(refinement): state against the concrete page-table encoding
     once satp/Sv39 activation lands; the trivial statement here only
     reserves the obligation's place in the build. *)
  exact I.
Qed.
