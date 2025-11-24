/*==============================================================================================================================

   Copyright � 2003 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : "Common"
   Description          : Common Configuration file

   Author               : RTT
   Version              : 1.0
   Creation Date        : 5/27/2003

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

`ifndef  COGNITUM_defines
`define  COGNITUM_defines

`define  XMOSI_WIDTH    1+1+8+8+8+8+32+32 // Raceway to Tile : 98 = {xresetn, xpush, xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
`define  XMISO_WIDTH    1+1+8+8+8+8+32+32 // Tile to Raceway : 98 = {xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}

`define  SMOSI_WIDTH    1+4+1+10+256      // Salsa to Work Memory : 272 = {sreq, sen[3:0],swr, sadr[9:0], swrd[255:0]}
`define  SMISO_WIDTH    4+256             // Work Memory to Salsa : 260 = {     sack[3:0],                sbrd[255:0]}

`define  TILEZERO_VROMSIZE   32768        // x1
`define  TILEZERO_FRAMSIZE   16384        // x2
`define  TILEZERO_SRAMSIZE   65536        // x2

`define  TILEONE_CODESIZE     8192
`define  TILEONE_DATASIZE     8192
`define  TILEONE_WORKSIZE    65536
// Memory decodes
`define  T0_ROM            5'b00_000      // decode address bits 31,30, 17,16,15         decode[31:30],decode[17:log2(MemSize)]
`define  T0_CM             6'b00_0010     // decode address bits 31,30, 17,16,15,14
`define  T0_DM             6'b00_0011     // decode address bits 31,30, 17,16,15,14
`define  T0_IM             4'b00_01       // decode address bits 31,30, 17,16
`define  T0_XM             1'b1           // decode address bit  31

`define  T1_CM             7'b00_00000    // decode address bits 31,30, 17,16,15,14,13
`define  T1_DM             7'b01_00000    // decode address bits 31,30, 17,16,15,14,13
`define  T1_WM             4'b10_00       // decode address bits 31,30, 17,16

// Raceway command codes
`define  SYSWR    8'b10010000             // System Unicast Write (to clock register)
`define  UNIWR    8'b10010001             // Unicast Write
`define  UNIWS    8'b1001?001             // Unicast Write or Swap
`define  UNIRD    8'b10001001             // Unicast Read
`define  UNIER    8'b00000000             // Unicast Error Acknowledge
`define  UNIWK    8'b00001001             // Unicast Write Acknowledge
`define  UNISK    8'b0001?001             // Unicast Write or Swap Acknowledge
`define  UNIRK    8'b00001010             // Unicast Read  Acknowledge
`define  BCTWR    8'b10110001             // Broadcast Write RAM
`define  BCTRL    8'b10100000             // Broadcast Release Reset
`define  BCTRS    8'b10111000             // Broadcast Set     Reset
// Raceway TILE addresses
`define  TILEZ    8'b00000000

`endif //  COGNITUM_defines

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
