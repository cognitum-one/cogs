//==============================================================================================================================
//
//   Copyright © 1996..2025 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Simple 2 port RAM interface with atomic-exchange operation
//
//===============================================================================================================================
//      THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//
//===============================================================================================================================

//`include "A2_project_settings.vh"

module   RAM_2Pax #(                                  // 2Pax : Two port atomic exchange
   parameter   DEPTH    = 2048,
   parameter   WIDTH    = 32,
   parameter   BASE_A   = 32'h0,
   parameter   BASE_B   = 32'h0
   )(
   output wire [    WIDTH + 2 -1:0] amiso,            // {agnt, aack, ardd[31:0]},  grant, acknowledge, 32-bit read-data
   output wire [    WIDTH + 2 -1:0] bmiso,            // {bgnt, back, brdd[31:0]},  grant, acknowledge, 32-bit read-data

   input  wire [2 * WIDTH + 3 -1:0] amosi,            // RAM   Port :  67 {areq, awr,ard,  awrd[31:0], aadr[31:0]}
   input  wire [2 * WIDTH + 3 -1:0] bmosi,            // RAM   Port :  67 {breq, bwr,brd,  bwrd[31:0], badr[31:0]}
   input  wire                      bacpt,            // b-side acknowledge has been accepted by xmiso interface
   input  wire                      clock
   );

//localparam     LG2D  = `LG2(DEPTH) + `LG2((WIDTH/8));       // e.g. 2048 -> 11   +   32 ->  2  = 13  address bits for 8k Bytes
localparam     LG2D  = 13;       // e.g. 2048 -> 11   +   32 ->  2  = 13  address bits for 8k Bytes

// Data Memory ==================================================================================================================

wire              eAM, wAM, rAM,  eBM, wBM, rBM;
wire  [WIDTH-1:0]  iAM, aAM, AMo,  iBM, aBM, ABo; //JLPL
reg                AMk,            BMk;
reg XC, XCA; //JLPL
// Decollate input buses
assign {eAM,wAM,rAM,iAM,aAM}   = amosi,
       {eBM,wBM,rBM,iBM,aBM}   = bmosi;

wire  Areq  =  eAM  & (aAM[31:LG2D] == BASE_A[31:LG2D]),
      Breq  =  eBM  & (aBM[31:LG2D] == BASE_B[31:LG2D]);

// Arbitrate -- well, give priority to the A side
wire              eDM   =  Areq | Breq  |  XC;                                                   // RAM enable
wire              xDM   =  Areq ?  wAM  &  rAM  :  Breq  &  wBM  &  rBM;                  // Decode Exchange (SWAP) //JLPL
wire              cDM   =  Areq ? ~wAM  & ~rAM  :  Breq  & ~wBM  & ~rBM;                  // Decode Read and Clear //JLPL
wire              wDM   = (Areq ?  wAM  & ~rAM  :  Breq  &  wBM  & ~rBM) | XC;            // RAM write enable
wire  [WIDTH-1:0] iDM   =  cDM  ?  32'h0
                        :  Areq || XCA  ?  iAM           :  iBM;
wire  [ LG2D-3:0] aDM   =  Areq || XCA  ?  aAM[LG2D-1:2] :  aBM[LG2D-1:2]; // address bits [12:2] //JLPL - Match aDM indexing to 10:0 for RAM devices for ncverilog compilation
//                         ****
wire  [WIDTH-1:0]  DMo;

wire dummy_out; //JLPL
wire tie_high = 1'b1; //JLPL
wire tie_low = 1'b0; //JLPL
wire clock_RAM;

//`ifdef   REAL_DEVICE //JLPL
`ifdef   synthesis    //JLPL
   PREICG_X2N_A7P5PP84TR_C14 iCGxx (.ECK(clock_RAM), .CK(clock), .E(eDM), .SE(1'b0));

   //rfsphse2KX32_mx8 i_DataRAM (
   rfsphse2KX32_mx8 i_DataRAM (
     `ifdef POWER_PINS   //JLPL
      .VDDCE (tie_high), //JLPL
     .VDDPE (tie_high), //JLPL
     .VSSE  (tie_low), //JLPL
    `endif             //JLPL
      .CLK  (clock_RAM),
      .CEN  (~eDM), //JLPL
      .GWEN (~wDM), //JLPL
      .A    ( aDM),
      .D    ( iDM),
      .STOV (1'b0),
      .EMA  (3'b010), //JLPL
      .EMAW (2'b00), //JLPL
      .EMAS (1'b0), //JLPL
      .RET1N(1'b1), //JLPL
      //.RET2N(1'b0),
     //.PRDYN(dummy_out), //JLPL
     //.PGEN(1'b0), //JLPL
      .Q    (DMo)
      );
`else
   preicg1 iCGxx (                      // RTT 20250727
      .ECK  (clock_RAM),
      .CK   (clock),
      .E    (eDM),
      .SE   (1'b0)
      );
RAM #(DEPTH,WIDTH) i_DataRAM ( //JLPL
   .RAMo  ( DMo[WIDTH-1:0]),
   .iRAM  ( iDM[WIDTH-1:0]),
   .aRAM  ( aDM[ LG2D-3:0]),
   .eRAMn (~eDM),
   .wRAMn (~wDM),
   .clock (clock_RAM)
   );
`endif

always @(posedge clock) XC    <= `tCQ  xDM  |  cDM;
always @(posedge clock) XCA   <= `tCQ (xDM  |  cDM) & Areq;
always @(posedge clock) AMk   <= `tCQ  Areq;
always @(posedge clock) BMk   <= `tCQ ~Areq &  Breq;

assign   amiso =  { ~XC,         {WIDTH+1{AMk}} & {1'b1, DMo}},                   // ~XC takes the grant away
         bmiso =  {~(XC | Areq), {WIDTH+1{BMk}} & {1'b1, DMo}};                   // ~XC or Areq takes the grant away

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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
