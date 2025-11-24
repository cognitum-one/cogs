//==============================================================================================================================
//
//   Copyright © 1996..2022 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Simple 1 port RAM interface
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
//    RAM_1P #(
//       .DEPTH   (4096),                          // 16384 Bytes
//       .WIDTH   (32),                            // 4 Bytes
//       .BASE    (32'h00000000),                  // Base Address
//       .WR_THRU (0),                             // Write Through   0: read OLD write new   1: write new and read new
//       .tACC    (1),                             // Access time in clock cycles     -- supports up to 7 wait states
//       .tCYC    (1)                              // RAM cycle time in clock cycles
//       ) inst (
//       .miso    (output_name [WIDTH+1:0]),       // RAM Output  34 = {gnt, ack, rdd[31:0]},
//       .mosi    (input_name[2*WIDTH+1:0]),       // RAM Input   66 = {req, wr,  wrd[31:0], adr[31:0]}
//       .reset   (reset),                         // Reset Acknowledge state machine
//       .clock   (clock)
//       );
//
//===============================================================================================================================

`ifdef synthesis
    `include "/proj/TekStart/lokotech/soc/users/romeo/cognitum_src_a0/src/include/A2_project_settings.vh"
`else
    `include "A2_project_settings.vh"
`endif

module   RAM_1P #(
   parameter   DEPTH    =  4096,                         // 16384 Bytes
   parameter   WIDTH    =  32,                           // 4 Bytes
   parameter   BASE     =  0,                            // Base Address -- must be a power of 2 boundary
   parameter   WR_THRU  =  0,                            // Write Through   0: read OLD write new   1: write new and read new
   parameter   tACC     =  1,                            // Access time in clock cycles     -- supports up to 7 wait states
   parameter   tCYC     =  1,                            // RAM cycle time in clock cycles
   parameter   LIMIT    = BASE + DEPTH                   // Address must be less than this value and greater than or equal to BASE
   )(
   output wire [WIDTH      :0]  miso,                    // RAM Output  34 = {gnt, ack, rdd[31:0]},
   input  wire [2*WIDTH  +1:0]  mosi,                    // RAM Input   66 = {req, wr,  wrd[31:0], adr[31:0]}
   input  wire                  reset,                   // Reset Acnowledge state machine
   input  wire                  clock
   );

localparam     DMSB  = 17,                            // Most  significant Address decode bit
               DLSB  = `LG2(DEPTH),                   // Least significant Address decode bit
               MMSB  = DLSB -1,                       // Most  significant Memory Address bit
               MLSB  = `LG2(WIDTH/8);                 // Least significant Memory Address bit (8-bit bytes)

// Nets -------------------------------------------------------------------------------------------------------------------------
wire  [31:0]      wrd;
wire  [31:0]      rdd;
wire  [31:0]      adr;
wire              req;
wire              gnt;
wire              wr;
wire              ack;

wire              eRAM;
wire              wRAM;
wire  [AMSB:ALSB] aRAM;
wire  [31:0]      iRAM, RAMo
;
// Decollate bus ----------------------------------------------------------------------------------------------------------------
assign   {req, wr, wrd[31:0], adr[31:0]}  =  mosi[`DMISO_WIDTH-1:0]; // 1+1+32+32 = 66

// RAM signals ------------------------------------------------------------------------------------------------------------------
assign   eRAM  =  req & ({adr[31:30],adr[DMSB:DLSB]} == BASE) & (WAK[IDLE] | WAK[CYCLE]),
         aRAM  =  adr[MMSB:MLSB],
         iRAM  =  awrd[31:0],
         wRAM  =  awr;

// RAM --------------------------------------------------------------------------------------------------------------------------
RAM #(0,DEPTH,WIDTH) i_RAM (
   .RAMo  ( RAMo[WIDTH-1:0]),
   .iRAM  (iRAM [WIDTH-1:0]),
   .aRAM  (aRAM [ LG2D-1:0]),
   .eRAM  (eRAM),
   .wRAM  (wRAM),
   .clock (clock)
   );

// Wait State Machine ----------------------------------------------------------------------------------------------------------
reg   [7:0] WAK; localparam  [3:0]  IDLE = 0, READ = tACC, CYCL = tCYC;
wire        pipe = (tACC == tCYC) & eROM;

always @(posedge clock)
   if (reset)                 WAK <= `tCQ 8'b00000001;
   else case(1'b1)
      WAK[IDLE] : if (eROM)   WAK <= `tCQ 8'b00000010;
      WAK[READ] : if (pipe)   WAK <= `tCQ 8'b00000010;
                else          WAK <= `tCQ {WAK[6:0],WAK[7]};
      WAK[CYCL] : if (eROM)   WAK <= `tCQ 8'b00000010;
                else          WAK <= `tCQ 8'b00000001;
      default                 WAK <= `tCQ {WAK[6:0],WAK[7]};
      endcase

assign   ack  =  WAK[READ];

// Recollate buse for output --------------------------------------------------------------------------------------------------
assign   miso =  {eRAM, ack, {32{ack}} & RAMo},

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
