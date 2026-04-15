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

`include "A2_project_settings.vh"

module   RAM_2P #(
   parameter   DEPTH = 2048,
   parameter   WIDTH = 32,
   parameter   TYPE  = 2'b00,                         // Base Address
   parameter   tACC  =  1,                            // Access time in clock cycles     -- supports up to 7 wait states
   parameter   tCYC  =  1                             // RAM cycle time in clock cycles
   )(
   output wire [`DMISO_WIDTH -1:0]  amiso,            // {agnt, aack, ardd[31:0]},  grant, acknowledge, 32-bit read-data
   output wire [`DMISO_WIDTH -1:0]  bmiso,            // {bgnt, back, brdd[31:0]},  grant, acknowledge, 32-bit read-data

   input  wire [`DMOSI_WIDTH -1:0]  amosi,            // RAM   Port :  66 {areq, awr,  awrd[31:0], aadr[31:0]}
   input  wire [`DMOSI_WIDTH -1:0]  bmosi,            // RAM   Port :  66 {breq, bwr,  bwrd[31:0], badr[31:0]}
   input  wire                      bacpt,            // b-side acknowledge has been accepted by xmiso interface
   input  wire                      reset,
   input  wire                      clock
   );

localparam     AMSB  = `LG2(DEPTH * (WIDTH/8)) -1,    // Most significant Address bit
               ALSB  = `LG2(WIDTH/8),                 // Least significant Address bit
               LG2D  = `LG2(DEPTH);                   // e.g. 4096 -> 12

// Nets -------------------------------------------------------------------------------------------------------------------------
reg   [ 3:0]   SSM;                          // Selection State Machine
reg   [ 7:0]   WSM;                          // Wait State Machine
reg            AOK, BOK;                     // Acknowledge 'hold' flops until 'aok' accepts it


wire  [31:0]   RAMo, iRAM;

wire  [LG2D-1:0]  aRAM;
wire              eRAM, wRAM;

wire  [31:0]   awrd, bwrd;
wire  [31:0]   aadr, badr;
wire           areq, breq;
wire           agnt, bgnt;
wire           awr,  bwr;
wire           aack, back;
wire           asel, bsel;
wire           reqA, reqB;
wire           mreq, mack;

// Decollate buses --------------------------------------------------------------------------------------------------------------
assign   {areq,awr,awrd[31:0],aadr[31:0]}  =  amosi[65:0],        // 1+1+32+32   =   66
         {breq,bwr,bwrd[31:0],badr[31:0]}  =  bmosi[65:0];        // 1+1+32+32   =   66

// Qualify  Requests
assign   reqA  =  areq & ~aadr[31:30],
         reqB  =  breq & (badr[31:30] == TYPE);

// FSM --------------------------------------------------------------------------------------------------------------------------
localparam  [1:0] IDLE = 0, AACC = 1, BACC = 2, STAY = 3;
// A has absolute priority over B!!!
always @(posedge clock)
   if (reset)                             SSM <= `tCQ 1'b1 << IDLE;
   else (* parallel_case *) case (1'b1)
      SSM[IDLE]   : if (reqA           )  SSM <= `tCQ 1'b1 << AACC;
               else if (reqB           )  SSM <= `tCQ 1'b1 << BACC;
      SSM[AACC]   : if (mack  &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (mack  && !reqA )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[BACC]   : if (mack  && !bacpt)  SSM <= `tCQ 1'b1 << STAY;
               else if (mack  &&  reqA )  SSM <= `tCQ 1'b1 << AACC;
               else if (mack  &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (mack           )  SSM <= `tCQ 1'b1 << IDLE;
      SSM[STAY]   : if (          reqA )  SSM <= `tCQ 1'b1 << AACC;
               else if (bacpt &&  reqB )  SSM <= `tCQ 1'b1 << BACC;
               else if (bacpt          )  SSM <= `tCQ 1'b1 << IDLE;
      endcase

assign   agnt  =  SSM[IDLE] | ~SSM[IDLE] & mack,
         bgnt  = ~reqA &  agnt,
         asel  =  reqA &  agnt,
         bsel  =  reqB &  bgnt,
         mreq  =  asel |  bsel;

// Multiplex between buses ------------------------------------------------------------------------------------------------------
assign   eRAM  =  asel |  bsel,
         aRAM  = {LG2D{asel}} & aadr[AMSB:ALSB]
               | {LG2D{bsel}} & badr[AMSB:ALSB],
         iRAM  = {  32{asel}} & awrd[31:0]
               | {  32{bsel}} & bwrd[31:0],
         wRAM  =       asel   & awr
               |       bsel   & bwr;

// RAM --------------------------------------------------------------------------------------------------------------------------
RAM #(1,DEPTH,WIDTH) i_DataRAM (
   .RAMo  ( RAMo[WIDTH-1:0]),
   .iRAM  (iRAM [WIDTH-1:0]),
   .aRAM  (aRAM [ LG2D-1:0]),
   .eRAM  (eRAM),
   .wRAM  (wRAM),
   .clock (clock)
   );

// Wait State Machine ----------------------------------------------------------------------------------------------------------
localparam  [2:0]  WAIT = 0,  LAST = 7;

always @(posedge clock)
   if (reset)                 WSM <= `tCQ 1'b1 << IDLE;
   else case(1'b1)
      WSM[WAIT] : if (mreq)   WSM <= `tCQ 1'b1 << (8 - tCYC);
      WSM[LAST] : if (mreq)   WSM <= `tCQ 1'b1 << (8 - tCYC);
                  else        WSM <= `tCQ 1'b1 << WAIT;
      default                 WSM <= `tCQ WSM  << 1'b1;
      endcase

assign   mack  =  WSM[8-tACC] |  SSM[WAIT],
         aack  =  mack        &  SSM[AACC],
         back  =  mack        &  SSM[BACC];

// Recollate buses for output --------------------------------------------------------------------------------------------------
assign   amiso =  {agnt, aack, {32{aack}} & RAMo},
         bmiso =  {bgnt, back, {32{back}} & RAMo};

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
