/* memory.x - Linker script for the STM32F446RETx */

MEMORY
{
  /* RAM begins at 0x20000000 and has a size of 128K */
  RAM (xrw)  : ORIGIN = 0x20000000, LENGTH = 128K

  /* Flash memory begins at 0x80000000 and has a size of 512K */
  FLASH (rx) : ORIGIN = 0x08000000, LENGTH = 512K
}
