/*==============================================================================================================================

   Copyright © 2003 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Edradour
   Description          : Configuration file

   Author               : RTT
   Version              : 1.0
   Creation Date        : 5/27/2021

================================================================================================================================
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
================================================================================================================================
   Notes for Use and Synthesis
==============================================================================================================================*/

`ifndef  EDRADOUR_DEFINES
`define  EDRADOUR_DEFINES

`include "A2Sv2_defines.vh"

//
// Raceway Fields                     98      97    96 95:88 87:80 79:72 71:64  63:32    31:0    = 99
//       xmosi[3*WA2S+3   :0] =  {xclock,xresetn,xpush,xcmd, xtag, xdst, xsrc, xwdata, xaddr }
//       xmosi_fb             =   xirdy
//
//       xmiso[3*WA2S+3   :0] =  {xclock,xresetn,xordy,xcmd, xtag, xdst, xsrc, xrdat0, xrdat1}
//       xmiso_fb             =   xpop
//
// Tile Fields                                      96 95:88 87:80 79:72 71:64  63:32    31:0    = 97
//       bmosi[3*WA2S+1   :0] =  {               bpush,bcmd, btag, bdst, bsrc, bwdata, baddr }
//       bmosi_fb             =   birdy
//
//       bmiso[3*WA2S+1   :0] =  {               bordy,bcmd, btag, bdst, bsrc, brdat0, brdat1}
//       bmiso_fb             =   bpop
//
//                                 47   46:45  44:13   12:0
//       jmosi[WIMM+WA2S-1:0] =  {eIO,   cIO,      T,  aIO };
//
// WM Salsa Fields               275 274:271  270  269:260  259:256       255:0
//       smosi[3*WA2S+3 :0] =  {sreq,sen[3:0],swr,sadr[9:0],sbs[3:0],swrd[255:0]}
//                               259:256       255:0
//       smiso[259      :0] =  {sack[3:0],srdd[255:0]}
//
// A2S Fields
//                              Enable,Write,WriteData,Address
//                                 65     64   63:32   31:0
//       cmosi[2+ 2*WA2S-1:0] =  {eCM,  1'b0,   IMM0,  aCM };
//       dmosi[2+ 2*WA2S-1:0] =  {eDM,   wDM,    iDM,  aDM };
//                                 47   46:45  44:13   12:0
//       imosi[WIMM+WA2S-1:0] =  {eIO,   cIO,      T,  aIO };
//
//                                33  31:0
//       cmiso[WA2S       :0] =  {CMk, CMo}
//       dmiso[WA2S       :0] =  {DMk, DMo}
//       imiso[WA2S       :0] =  {IOk, IOo}
//

`define  RW_WIDTH 99                      // Raceway Width
`define  BM_WIDTH 97                      // BMOSI Width -- i.e no bundled clock or reset

`define  RW_CLK   98                      // Raceway Clock
`define  RW_RSTN  97                      // Raceway Resetn
`define  RW_PUSH  96                      // Raceway push      MOSI
`define  RW_ORDY  96                      // Output  ready     MISO
`define  RW_BNDL  95:0                    // Bundle

`define  RW_WORD2 95:64
   `define  RW_CMTG  95:64
      `define  RW_CMD   95:88             // Command
      `define  RW_REQ   95                // Request
      `define  RW_NAK   95                // Acknowledge_NOT
      `define  RW_ERR   94                // Error
      `define  RW_RSP   93:88             // Response
      `define  RW_BCST  93                // Broadcast
      `define  RW_RDWR  92                // ~Read / Write
      `define  RW_CMND  92:91             // Command
         `define  RW_RDCL  2'b00
         `define  RW_READ  2'b01
         `define  RW_WRIT  2'b10
         `define  RW_SWAP  2'b11
      `define  RW_SIZ   90:88             // Transfer Size
      `define  RW_TAG   87:80             // Tag byte
      `define  RW_DST   79:72             // On-chip Destination Tile
      `define  RW_DSTU  79:75             // Destination Upper Address : Column of Tiles
      `define  RW_DSTL  74:72             // Destination Lower Address : Tile within column
      `define  RW_DSTT  78:76             // Destination Lower Address : Tile row within a column //Joel 12/1923
      `define  RW_SRC   71:64             // On-chip Source Tile

`define  RW_WORD1 63:32
   `define  RW_DAT0  63:32                // Write Word or Read Word-0

`define  RW_WORD0 31:00                   // Address or Read Word-1 (Work RAM Access onlt)
   `define  RW_ADDR  31:00
   `define  RW_DAT1  31:00
   `define  RW_IADR  14:02                // IO Register addresses -- 4-byte
   `define  RW_MSEL  31:30                // Upper 2 Address bits
      `define  RW_CODE  2'b00
      `define  RW_DATA  2'b01
      `define  RW_WORK  2'b10
      `define  RW_REGS  2'b11

// Raceway Command codes
`define  SYSWR    8'b10010000       // System Unicast Write (to clock register)
`define  UNIWR    8'b10010001       // Unicast Write
`define  UNIWS    8'b1001?001       // Unicast Write or Swap
`define  UNIRD    8'b10001001       // Unicast Read
`define  UNIER    8'b00000000       // Unicast Error Acknowledge
`define  UNIWK    8'b00001001       // Unicast Write Acknowledge
`define  UNISK    8'b0001?001       // Unicast Write or Swap Acknowledge
`define  UNIRK    8'b00001010       // Unicast Read  Acknowledge
`define  BCTWR    8'b10110001       // Broadcast Write RAM
`define  BCTRL    8'b10100000       // Broadcast Release Reset
`define  BCTRS    8'b10111000       // Broadcast Set     Reset
// Raceway TILE addresses
`define  TILEZ    8'b00000000

// Salsa SMISO\SMOSI fields ==========================================================================================

`define  SMOSI_WIDTH 276
`define  SMOSI_RQ    275               // Request
`define  SMOSI_EN    274:271           // Word Write mask per 256-bit double bank of 128-bit banks
`define  SMOSI_EN3   274
`define  SMOSI_EN2   273
`define  SMOSI_EN1   272
`define  SMOSI_EN0   271
`define  SMOSI_WR    270               // Write enable
`define  SMOSI_AD    269:260           // Address within block
`define  SMOSI_BS    259:256           // 4 X 256 Bank Select
`define  SMOSI_DW    255:0             // Write Data

`define  SMISO_WIDTH 260
`define  SMISO_AK    259:256           // Acknowledge per bank
`define  SMISO_DR    255:0             // Double Read  Data
// Work Memory Address fields
`define  WM_BPW   1:0                  // Address: Bytes  per  Word : Always zero!
`define  WM_WPB   3:2                  // Address: Words  per  Bank
`define  WM_ODD     4                  // Address: Odd (Upper) Bank
`define  WM_DPB   4:3                  // Address: Double words per Bank (MOSI Reads)
`define  WM_BNK   6:5                  // Address: Bank Select
`define  WM_RAD  15:7                  // Address: Row Address

// Register Addressing ===========================================================================================

// Tile -- IMOSI Address
`define  SHA256_base       13'h1000
`define  SHA256_MESG       13'h1000
`define  SHA256_HASH       13'h1001
`define  SHA256_STOP       13'h1002
`define  SHA256_STRT       13'h1003
`define  SHA256_INIT       13'h1004

`define  XSALSA_base       13'h1008
`define  XSALSA_XBUF       13'h1008
`define  XSALSA_XBBS       13'h1009
`define  XSALSA_STOP       13'h100a
`define  XSALSA_STRT       13'h100b

`define  GATEWY_base       13'h1010
`define  GATEWY_TIMR       13'h1010
`define  GATEWY_ADDR       13'h1011
`define  GATEWY_DATA       13'h1012
`define  GATEWY_CMND       13'h1013
`define  GATEWY_BUSY       13'h1014
`define  GATEWY_RDA1       13'h1015
`define  GATEWY_RDA0       13'h1016
`define  GATEWY_RESP       13'h1017
`define  GATEWY_TIMO       13'h1018
`define  GATEWY_MyID       13'h1019

`define  CLOCKS_base       13'h1020
`define  CLOCKS_READ       13'h1020

`define  aMTG              13'h1030
`define  aSCR              13'h1555

// xmosi\bmosi addresses -- DERIVED from imosi addresses ---( don't touch ) -------------------------------------------------------------

`define  TILE_SHA256_MESG  {17'h_18000,`SHA256_MESG,2'b00}
`define  TILE_SHA256_HASH  {17'h_18000,`SHA256_HASH,2'b00}
`define  TILE_SHA256_STOP  {17'h_18000,`SHA256_STOP,2'b00}
`define  TILE_SHA256_STRT  {17'h_18000,`SHA256_STRT,2'b00}
`define  TILE_SHA256_INIT  {17'h_18000,`SHA256_INIT,2'b00}

`define  TILE_XSALSA_XBUF  {17'h_18000,`XSALSA_XBUF,2'b00}
`define  TILE_XSALSA_XBBS  {17'h_18000,`XSALSA_XBBS,2'b00} // Byte Swap
`define  TILE_XSALSA_STOP  {17'h_18000,`XSALSA_STOP,2'b00}
`define  TILE_XSALSA_STRT  {17'h_18000,`XSALSA_STRT,2'b00}

`define  TILE_GATEWY_ADDR  {17'h_18000,`GATEWY_ADDR,2'b00}
`define  TILE_GATEWY_DATA  {17'h_18000,`GATEWY_DATA,2'b00}
`define  TILE_GATEWY_CMND  {17'h_18000,`GATEWY_CMND,2'b00}
`define  TILE_GATEWY_TIMO  {17'h_18000,`GATEWY_TIMO,2'b00}
`define  TILE_GATEWY_RDA1  {17'h_18000,`GATEWY_RDA1,2'b00}
`define  TILE_GATEWY_RDA0  {17'h_18000,`GATEWY_RDA0,2'b00}
`define  TILE_GATEWY_RESP  {17'h_18000,`GATEWY_RESP,2'b00}
`define  TILE_GATEWY_BTGS  {17'h_18000,`GATEWY_BTGS,2'b00}
`define  TILE_GATEWY_BUSY  {17'h_18000,`GATEWY_BUSY,2'b00}

`define  TILE_CLOCKS_READ  {17'h18000,`CLOCKS_READ,2'h0}
`define  TILE_SET_CLOCKS    32'hffff_0000
`define  TILE_RESET         32'hffff_ffff
`define  TILE_RELEASE       32'hffff_8000

// Local DMA NEWS
`define  LMOSI_WIDTH       66
`define  LMOSI_EN          65
`define  LMOSI_WR          64
`define  LMOSI_DW          63:32
`define  LMOSI_MS          31:30                   // Memory Select Address (CM,DM,WM,IO)
`define  LMOSI_AD          31:0
`define  LMISO_WIDTH  33
`define  LMISO_DR          31:0
`define  LMISO_AK          32
`endif

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////