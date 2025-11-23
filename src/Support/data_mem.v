//==============================================================================================================================
//
//   Copyright © 1996..2025 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Simple 2 port Data RAM with 1-RO 1-RW
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
//
//===============================================================================================================================

module   data_mem #(
   parameter   DEPTH    = 16384,
   parameter   WIDTH    = 32,
   parameter   LG2D     = 14,
   parameter   BASE     = 32'h00010000
   )(
   output wire [  WIDTH+1:0] cmiso,    // RAM Output  :  34 {agnt, aack, ardd[31:0]}
   output wire [  WIDTH+1:0] dmiso,    // RAM Output  :  34 {bgnt, back, brdd[31:0]}

   input  wire [  WIDTH  :0] cmosi,    // RAM RO Port :  33 {areq,                       aadr[31:0]}  // read only
   input  wire [2*WIDTH+2:0] dmosi,    // RAM RW Port :  66 {breq, bwr, brd, bwrd[31:0], badr[31:0]}
   input  wire               clock
   );

// Decollate buses --------------------------------------------------------------------------------------------------------------
reg               CMk,  DMk;
wire              creq, dreq;
wire              drd,  dwr;
wire  [WIDTH-1:0]       dwrd;
wire  [WIDTH-1:0] cadr, dadr;

assign {creq,             cadr}   = cmosi,
       {dreq,dwr,drd,dwrd,dadr}   = dmosi;

// Arbitrate -- well, give priority to the A side -------------------------------------------------------------------------------

wire              reqC  =  creq & (cadr[31:LG2D+2] == BASE[31:LG2D+2]);
wire              reqD  =  dreq & (dadr[31:LG2D+2] == BASE[31:LG2D+2]);
wire              eDM   =  reqC | reqD;                                          // RAM enable
wire              wDM   =  reqC ? 1'b0 : dwr;                                    // RAM write enable
wire  [LG2D -1:0] aDM   =  reqD ? dadr[LG2D+1:2] : cadr[LG2D+1:2];               // RTT 20250827 //JLPL - 8_26_25
wire  [WIDTH-1:0] iDM   =  dwrd;
wire              clock_RAM;
wire  [WIDTH-1:0] DMo;

// The actual RAM ---------------------------------------------------------------------------------------------------------------

`ifdef   synthesis
   PREICG_X2N_A7P5PP84TR_C14 iCGxx (.ECK(clock_DM), .CK(clock), .E(eDM), .SE(1'b0));
   SRAM_16KX32M16B2 i_SRAM (.Q(DMo), .CLK(clock_DM), .CEN(eDM), .GWEN(wDM), .A(aDM), .D(iDM),
                       .STOV(1'b0), .EMA(3'b011), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1),
                       .WABL(1'b0), .WABLM(2'b00) );



`else

   preicg iCG (
      .ECK  (clock_RAM),
      .CK   (clock),
      .E    (eDM),
      .SE   (1'b0)
      );
   RAM #(DEPTH,WIDTH) i_RAM (
      .RAMo  ( DMo[WIDTH-1:0]),
      .iRAM  ( iDM[WIDTH-1:0]),
      .aRAM  ( aDM[ LG2D-1:0]), //JLPL - 8_26_25
      .eRAMn (~eDM),
      .wRAMn (~wDM),
      .clock (clock_RAM)
      );
`endif

// Acknowledge and Output -------------------------------------------------------------------------------------------------------

always @(posedge clock) DMk <= `tCQ  reqD;
always @(posedge clock) CMk <= `tCQ ~reqD & reqC;

assign
   dmiso =  {  reqD,          {WIDTH+1{DMk}} & {1'b1, DMo}},
   cmiso =  { ~reqD & reqC,   {WIDTH+1{CMk}} & {1'b1, DMo}};

endmodule

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
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
