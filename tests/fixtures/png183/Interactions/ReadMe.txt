Tests various edge case interactions in the GBA.

Internal_Cycle_DMA_IRQ tests what happens when an IRQ occurs in the middle of a DMA when paused on an internal cycle where the cpu can run. It seems that the IRQ pipeline is stalled along with the cpu, because an extra instruction is run after the DMA finishes before IRQ starts.
