//===============================================================================================================================
//
//    Copyright © 2022 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to Authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : Newport ASIC
//
//    Description          : Column Vector or Array of Scrypt Tiles with Interconnect
//
//===============================================================================================================================
//    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//    NOTES:
//                  +------+              +------+              +------+
//       ==data====>|      |=====data====>|      |=====data====>|      |=====data=
//                  |      |              |      |              |      |
//       --give---->|wr  or|-----give---->|wr  or|-----give---->|wr  or|-----give-
//                  |      |              |      |              |      |
//       <-take-----|ir  rd|<----take-----|ir  rd|<----take-----|ir  rd|<----take-
//                  +------+              +------+              +------+
//
//===============================================================================================================================

`include "NEWPORT_defines.vh"


module   Column #(
   parameter   [  4:0]  colID = 5'd1   //  Derived from previous level of hierachy
   )(
   // Raceway Output                   //          95:88     87:80     79:72     71:64     63:32      32:0
   output wire [  3:0]  CO_or,         //
   output wire [383:0]  CO,            // 4 x 96 { cmd[7:0], tag[7:0], dst[7:0], src[7:0], wrd[31:0], adr[31:0]}
   input  wire [  3:0]  CO_rd,
   output wire          CO_ck,
   // Raceway Input
   input  wire [  3:0]  CI_wr,
   input  wire [383:0]  CI,            // 4 x 96 { cmd[7:0], tag[7:0], dst[7:0], src[7:0], wrd[31:0], adr[31:0]}
   output wire [  3:0]  CI_ir,
   input  wire          CI_ck,
   // Global
`ifdef DEBUG
   output wire [  7:0]  fini,
   output wire [ 32:0]  imiso,         // Slave data out {                IOk, IOo[31:0]}
   input  wire [ 47:0]  imosi,         // Slave data in  {cIO[2:0], iIO[31:0], aIO[12:0]}
   input  wire [  7:0]  go,
`endif
   output wire [  3:0]  PUFosc,
   output wire [  7:0]  TIMOo,
   output wire [  7:0]  FLAGo,
   input  wire [  7:0]  FLAGi,
   input  wire          reset
   );

// Reset ------------------------------------------------------------------------------------------------------------------------
wire        col_reset;
A2_reset i_rst (.q(col_reset), .reset(reset), .clock(CI_ck));   // Synchronise    to Incoming clock

// Collate and Decollate --------------------------------------------------------------------------------------------------------

// Raceway is the Master and the Tiles are Slaves!
wire  [  7:0] RT_ck;    // xmosi_clk    1
wire  [783:0] RT;       // xmosi       98 = {xresetn, xpush, xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
wire  [  7:0] RT_fb;    // xmosi_fb     1 =           xirdy

wire  [  7:0] TR_ck;    // xmiso_clk    1
wire  [783:0] TR;       // xmiso       98 = {xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
wire  [  7:0] TR_fb;    // xmiso_fb     1 =           xpop

wire  [ 31:0] CPO;      // Column PUF Osc

// Tiles ------------------------------------------------------------------------------------------------------------------------

Raceway  #(colID) iRW1 (
   // System Interconnect : Output from Raceway to Raceway
   .CO_rd (CO_rd),
   .CO    (CO),
   .CO_or (CO_or),
   .CO_ck (CO_ck),
   // System Interconnect : Inputs to Raceway from Raceway
   .CI    (CI),
   .CI_wr (CI_wr),
   .CI_ir (CI_ir),
   .CI_ck (CI_ck),
   // Local Interconnect  : Outputs from Raceway to Tiles
   .RT    (RT),
   .RT_fb (RT_fb[7:0]),
   .RT_ck (RT_ck[7:0]),
   // Local Interconnect  : Input to Raceway from Tiles
   .TR    (TR),
   .TR_fb (TR_fb[7:0]),
   .TR_ck (TR_ck[7:0]),
   // Global
   .reset (col_reset)
   );

genvar i;

`ifdef   DEBUG
wire  [32:0]   imiso_t  [0:7];
`endif

for (i=0;i<8;i=i+1) begin : Tiles
   localparam [7:0] tileID = {colID[4:0],i[2:0]};
`ifdef   DEBUG
   TileOne #(8192,8192,65536,tileID) iT01 (
`else 
`ifdef synthesis
   TileOne iT01 (
`else synthesis
   TileOne #(8192,8192,65536) iT01 (
`endif
`endif
      // Output Interface
      .xmiso_clk  (TR_ck[i]),                // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
      .xmiso      (TR[98*i+:98]),            // xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
      .xmiso_fb   (TR_fb[i]),                //          xpop
      // Input Interface
      .xmosi_clk  (RT_ck[i]),                // 97       96     95:88     87:80     79:72     71:64     63:32      31:0
      .xmosi      (RT[98*i+:98]),            // xresetn, xpush, xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
      .xmosi_fb   (RT_fb[i]),                //          xirdy
`ifdef   DEBUG
      .intpt_fini (fini[i]),
      .imiso      (imiso_t[i]),
      .imosi      (imosi),
      .go         (go[i]),
`endif
      // Globals
      .puf_osc    (CPO[4*i+:4]),
      .flag_out   (FLAGo[i]),
      .flag_in    (FLAGi[i]),
      .myID       ({colID[4:0],i[2:0]})
      );
   assign
      TIMOo[i]    =  TR[98*i+97];
   end

assign PUFosc[3:0]   =  CPO[31:28] | CPO[27:24] | CPO[23:20] | CPO[19:16] | CPO[15:12] | CPO[11:8]  | CPO[7:4]   | CPO[3:0];

`ifdef   DEBUG
assign imiso         =  imiso_t[0] | imiso_t[1] | imiso_t[2] | imiso_t[3] | imiso_t[4] | imiso_t[5] | imiso_t[6] | imiso_t[7];
`endif

endmodule

////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
