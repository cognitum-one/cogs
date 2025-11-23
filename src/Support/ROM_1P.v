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
//    ROM_1P #(
//       .DEPTH   (4096),
//       .WIDTH   (32),
//       .BASE    (32'h00000000),
//       .tACC    (1),
//       .tCYC    (1)
//       ) inst (
//       .miso    (output_name[WIDTH+1:0]),
//       .mosi    (input_name [WIDTH  :0]),
//       .reset   (reset),
//       .clock   (clock)
//       );
//
//===============================================================================================================================

`ifdef synthesis
     `include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/A2_project_settings.vh"
`else
     `include "A2_project_settings.vh"
`endif

module   ROM_1P #(
   parameter   DEPTH = 32768,                         // 32768 Bytes
   parameter   WIDTH = 32,                            // 4 Bytes
   parameter   BASE  =  0,    ,                       // Base Address
   parameter   tACC  =  2,                            // ROM access time in clock cycles     -- supports up to 6 wait states
   parameter   tCYC  =  4,                            // ROM cycle  time in clock cycles     -- supports up to 7 clock cycles
   parameter   LIMIT = BASE + DEPTH                   // Address must be less than this value and greater than or equal to BASE
   )(
   output wire [`CMISO_WIDTH -1:0]  miso,             // {grnt, ack, rdd[31:0]}     -- grant, acknowledge, 32-bit read-data
   input  wire [`CMOSI_WIDTH -1:0]  mosi,             // { req,      adr[31:0]}     -- request,  address
   input  wire                      reset,
   input  wire                      clock
   );

localparam     DMSB  = 17,                            // Most  significant Address decode bit
               DLSB  = `LG2(DEPTH),                   // Least significant Address decode bit
               MMSB  = DLSB -1,                       // Most  significant Memory Address bit
               MLSB  = `LG2(WIDTH/8);                 // Least significant Memory Address bit (8-bit bytes)

// Nets -------------------------------------------------------------------------------------------------------------------------
wire  [WIDTH-1:0] RAMo,
wire  [31:0]      adr;
wire              req;
wire              gnt;
wire              ack;

// Decollate bus ----------------------------------------------------------------------------------------------------------------
assign   {req, adr[31:0]}  =  mosi[`CMOSI-1:0];        // 1+32   =   33

// RAM signals ------------------------------------------------------------------------------------------------------------------
assign   eROM  =  req & ({adr[31:30],adr[DMSB:DLSB]} == BASE) & (WAK[IDLE] | WAK[CYCLE]),
         aROM  =  adr[MMSB:MLSB],

// ROM --------------------------------------------------------------------------------------------------------------------------
reg   [WIDTH-1:0] VIAS  [0:DEPTH-1];

always @(posedge clock)
   if (eROM)
      ROMo  <= `tCQ  VIAS[aROM];

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
assign   gnt  =  eROM

// Recollate buse for output --------------------------------------------------------------------------------------------------
assign   miso =  {gnt, ack, {32{ack}} & ROMo},

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
