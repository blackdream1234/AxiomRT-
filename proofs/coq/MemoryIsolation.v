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

(* ----------------------------------------------------------------- *)
(* Sv39 refinement note (AXIOM-MEMHW-012).
   v0.2 activated the Sv39 MMU (kernel/src/arch/riscv64/sv39.rs,
   paging.rs, paging_hw.rs; docs/12_MMU_SV39.md). The concrete leaf-PTE
   encoding is intended to realize MEM-P1: the U bit of a leaf PTE
   corresponds to the model's USER accessibility, and no kernel frame is
   ever encoded with the U bit set.

   We model the relevant PTE fields abstractly and state the two
   refinement properties the Rust `Pte::leaf` constructor already
   enforces by construction (rejecting user W^X and never setting U on a
   kernel mapping). These are stated here; the full machine-checked
   refinement against the extracted Rust remains TODO (see
   docs/11_VERIFICATION_PLAN.md §2, row "Memory isolation").          *)

Record LeafPte := mkLeaf {
  pte_user : bool;     (* U bit *)
  pte_write : bool;    (* W bit *)
  pte_exec : bool;     (* X bit *)
  pte_kernel_frame : bool (* frame lies in the kernel physical range *)
}.

(* The encoding invariant the constructor guarantees (sv39.rs::leaf):
   - a kernel-frame leaf never carries the user bit (MEM-P1 encoding);
   - a user leaf is never simultaneously writable and executable
     (MEM-P5 encoding). *)
Definition pte_wellformed (p : LeafPte) : Prop :=
  (pte_kernel_frame p = true -> pte_user p = false)
  /\ (pte_user p = true -> ~ (pte_write p = true /\ pte_exec p = true)).

(* MEM-P1 at the encoding level: a well-formed leaf mapping a kernel
   frame is not user-accessible. Discharged directly from the invariant;
   the remaining obligation is that `Pte::leaf` always yields a
   well-formed PTE (enforced in Rust, TODO to mirror in Coq).          *)
Theorem sv39_kernel_frame_not_user :
  forall p, pte_wellformed p -> pte_kernel_frame p = true -> pte_user p = false.
Proof.
  intros p [Hk _] Hkf. exact (Hk Hkf).
Qed.

Theorem sv39_user_leaf_not_wx :
  forall p, pte_wellformed p -> pte_user p = true ->
    ~ (pte_write p = true /\ pte_exec p = true).
Proof.
  intros p [_ Hwx] Hu. exact (Hwx Hu).
Qed.

Theorem sv39_encoding_refines_memp1 : True.
Proof.
  (* TODO(refinement): connect pte_wellformed to the Rust
     kernel/src/arch/riscv64/sv39.rs::leaf constructor (which rejects
     every ill-formed combination) and to the AddressSpace model of this
     file, so that MEM-P1 transfers to the running Sv39 tables. *)
  exact I.
Qed.
