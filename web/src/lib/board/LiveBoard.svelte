<script>
  // The live board view drew a .kicad_pcb with UltraRoute Studio's core and board-draw, built
  // from another repository. That is gone; the view returns on Boardroom's own `render` crate
  // (docs/one-system-spec.md, step 3). Its last version is in git history before that step.
  let { project, board } = $props();
</script>

<div class="live">
  <p class="note">
    The live board view of <b>{board?.name ?? project?.name ?? 'this board'}</b> is being rebuilt on
    Boardroom's own renderer (step 3 of <code>docs/one-system-spec.md</code>). Plots and 3D still
    show the published viewer.
  </p>
</div>

<style>
  .live { display: grid; place-items: center; height: 100%; }
  .note { max-width: 34rem; color: var(--muted, #888); line-height: 1.5; }
</style>
