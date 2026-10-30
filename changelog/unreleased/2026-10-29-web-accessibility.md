- **Web content is available to browser accessibility tools.** The WebGPU shell projects
  retained content into persistent native browser controls. Names, checked state, inline
  links, disabled controls and modal focus reuse Rune's existing owners. Rune Kit supplies
  field names and expansion state from its existing builders. Editors retain focus,
  selection and composition across unrelated commits and virtual-row unmounts. Browsers
  without `moveBefore` restore focus and selection but can cancel composition on reparenting.
  Secure values stay out of the DOM. Web exports can include page metadata and now
  preload wasm and generate compressed assets for static hosting. See `WEB.md` for
  authoring requirements, checks and platform limits; this change does not certify
  VoiceOver or WCAG conformance.
