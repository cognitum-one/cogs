//==============================================================================================================================
//
//   Copyright © 1996..2022 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Simple 3 port RAM interface
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
`ifdef synthesis
     `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_src_a0/src/include/A2_project_settings.vh"
`else
      `include "A2_project_settings.vh"
`endif

module   RAM_3P #(
   parameter   DEPTH = 4096,
   parameter   WIDTH = 32,
   parameter   BASE  = 18'h0,
   parameter   AMSB  = 17,                   // Most significant address bit for decode
   parameter   tACC  =  1,                   // Access time in clock cycles     -- supports up to 7 wait states
   parameter   tCYC  =  1                    // RAM cycle time in clock cycles
   )(
   output wire [33:0]  amiso,                // {agnt, aack, ardd[31:0]},  grant, acknowledge, 32-bit read-data
   output wire [33:0]  bmiso,                // {bgnt, back, brdd[31:0]},  grant, acknowledge, 32-bit read-data
   output wire [33:0]  cmiso,                // {cgnt, cack, crdd[31:0]},  grant, acknowledge, 32-bit read-data

   input  wire [65:0]  amosi,                // {areq, awr,  awrd[31:0], aadr[31:0]},  request, write, write-data, address
   input  wire [65:0]  bmosi,                // {breq, bwr,  bwrd[31:0], badr[31:0]},  request, write, write-data, address
   input  wire [65:0]  cmosi,                // {creq, cwr,  cwrd[31:0], cadr[31:0]},  request, write, write-data, address
   input  wire         reset,
   input  wire         clock
   );

localparam     LG2D  = `LG2(DEPTH),          // e.g. 4096 -> 12
               LG2W  = `LG2(WIDTH/8),        //        32 ->  2
               ADLO  =  LG2D + LG2W;         // Lowest address bit for upper bits compare

// Nets -------------------------------------------------------------------------------------------------------------------------
reg   [ 6:0]   SSM;
reg   [ 7:0]   RSM;

wire  [31:0]   RAMo, aRAM, iRAM;


wire  [31:0]   aadr, badr, cadr;
wire  [31:0]   awrd, bwrd, cwrd;
wire  [31:0]   ardd, brdd, crdd;
wire           areq, breq, creq;
wire           agnt, bgnt, cgnt;
wire           awr,  bwr,  cwr;
wire           aack, back, cack;
wire           mreq, mack;
wire           reqA, reqB, reqC;

// Decollate buses --------------------------------------------------------------------------------------------------------------

assign   {areq, awr, awrd[31:0], aadr}  =  amosi[65:0],                       // 1+1+32+16   =   50
         {breq, bwr, bwrd[31:0], badr}  =  bmosi[65:0],                       // 1+1+32+16   =   50
         {creq, cwr, cwrd[31:0], cadr}  =  cmosi[63:0];                       // 1+1+32+16   =   50

// Generate requests
assign   reqA  =  areq & (aadr[AMSB:ADLO] == BASE[AMSB:ADLO]),
         reqB  =  breq & (badr[AMSB:ADLO] == BASE[AMSB:ADLO]),
         reqC  =  creq & (cadr[AMSB:ADLO] == BASE[AMSB:ADLO]);

// States for FSM
localparam  [1:0] IDLE = 0, AACC = 1, BACC = 2, CACC = 3;
// A has absolute priority over B, B has absolute priority over C !!!
always @(posedge clock)
   if (reset)                          SSM <= `tCQ 1'b1 << IDLE;
   else (* parallel_case *) case (1'b1)
      SSM[IDLE]   : if (reqA)          SSM <= `tCQ 1'b1 << AACC;
               else if (reqB)          SSM <= `tCQ 1'b1 << BACC;
               else if (reqC)          SSM <= `tCQ 1'b1 << CACC;
      default       if (mack &&  reqA) SSM <= `tCQ 1'b1 << AACC;
               else if (mack &&  reqB) SSM <= `tCQ 1'b1 << BACC;
               else if (mack &&  reqC) SSM <= `tCQ 1'b1 << CACC;
               else if (mack)          SSM <= `tCQ 1'b1 << IDLE;
      endcase

assign   agnt  =  SSM[IDLE]                  | mack &                 ~SSM[IDLE],
         bgnt  =  SSM[IDLE] & ~reqA          | mack & ~reqA &         ~SSM[IDLE],
         cgnt  =  SSM[IDLE] & ~reqA & ~reqB  | mack & ~reqA & ~reqB & ~SSM[IDLE],
         asel  =  reqA & agnt,
         bsel  =  reqB & bgnt,
         csel  =  reqC & cgnt,
         mreq  =  asel | bsel | csel;

assign   eRAM  =  mreq,
         aRAM  = {32{asel}} & aadr[ADLO-1:LG2W]
               | {32{bsel}} & badr[ADLO-1:LG2W]
               | {32{csel}} & cadr[ADLO-1:LG2W],
         iRAM  = {32{asel}} & awrd[31:0]
               | {32{bsel}} & bwrd[31:0]
               | {32{csel}} & cwrd[31:0],
         wRAM  =     asel   & awr
               |     bsel   & bwr
               |     csel   & cwr;
`ifdef synthesis
     //sram_sphd_4KX32 i_DataRAM (.Q(RAMo), .CLK(clock), .CEN(eRAM), .GWEN(wRAM), 
                                //.A(aRAM), .D(iRAM), .STOV(1'b0), .EMA(3'b011), .EMAW(2'b0), 
                                //.EMAS(1'b0), .RET1N(1'b0), .WABL(1'b0), .WABLM(2'b0)
     //);
     SRAM_4KX32_M16 i_DataRAM (.Q(RAMo), .CLK(clock), .CEN(eRAM), .GWEN(wRAM),
                               .A(aRAM), .D(iRAM), .STOV(1'b0), .EMA(3'b011), .EMAW(2'b0),
                               .EMAS(1'b0), .RET1N(1'b0), .WABL(1'b0), .WABLM(2'b0)
     );
`else
    RAM #(1,DEPTH,WIDTH) i_DataRAM (
                                   .RAMo  ( RAMo[WIDTH-1:0]),
                                   .iRAM  (iRAM [WIDTH-1:0]),
                                   .aRAM  (aRAM [ LG2D+1:2]),
                                   .eRAM  (eRAM),
                                   .wRAM  (wRAM),
                                   .clock (clock)
     );
`endif

localparam  [2:0]  WAIT = 0, LAST = 7;

always @(posedge clock)
   if (reset)                 RSM <= `tCQ 1'b1 << IDLE;
   else case(1'b1)
      RSM[WAIT] : if (mreq)   RSM <= `tCQ 1'b1 << (8 - tCYC);
      RSM[LAST] : if (mreq)   RSM <= `tCQ 1'b1 << (8 - tCYC);
                  else        RSM <= `tCQ 1'b1 << WAIT;
      default                 RSM <= `tCQ RSM  << 1'b1;
      endcase

assign   mack  =  RSM[8-tACC],
         aack  =  SSM[AACC] &  mack,
         cack  =  SSM[BACC] &  mack,
         back  =  SSM[CACC] &  mack;

// Collate buses for output
assign   amiso =  {agnt, aack, {32{aack}} & RAMo},
         bmiso =  {bgnt, back, {32{back}} & RAMo},
         cmiso =  {cgnt, cack, {32{cack}} & RAMo};

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
