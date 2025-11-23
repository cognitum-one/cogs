//==============================================================================================================================
//
//   Copyright © 1996..2025 Advanced Architectures
//
//   All rights reserved
//   Confidential Information
//   Limited Distribution to authorized Persons Only
//   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//   Project Name         : Simple 3 port RAM interface with atomic-exchange operation -- A-port supports double width reads
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

module   RAM_3Pax #(
   parameter   DEPTH    = 4096,
   parameter   WIDTH    = 32,
   parameter   BASE     = 32'h80000000           // Set as Work Memory
   )(
   output wire [2 * WIDTH+1:0] amiso,            // RAM A Port : 66 {agnt, aack, ardd[63:0]},
   output wire [    WIDTH+1:0] bmiso,            // RAM B Port : 34 {bgnt, back, brdd[31:0]},
   output wire [    WIDTH+1:0] cmiso,            // RAM C Port : 34 {bgnt, back, brdd[31:0]},

   input  wire [2 * WIDTH+2:0] amosi,            // RAM A Port : 67 {areq, awr,ard,  awrd[31:0], aadr[31:0]}
   input  wire [2 * WIDTH+2:0] bmosi,            // RAM B Port : 67 {breq, bwr,brd,  bwrd[31:0], badr[31:0]}
   input  wire [2 * WIDTH+2:0] cmosi,            // RAM C Port : 67 {breq, bwr,brd,  bwrd[31:0], badr[31:0]}
   input  wire                 clock
   );

// Data Memory ==================================================================================================================

wire              eAM, wAM, rAM,  eBM, wBM, rBM,   eCM, wCM, rCM;
wire  [WIDTH-1:0] iAM, aAM, AMo,  iBM, aBM, ABo,   iCM, aCM, ACo;
reg               AMk,            BMk,             CMk;
reg               XCA,            XCB,             XCC;
reg               XC,  ADz;

// Decollate input buses
assign {eAM,wAM,rAM,iAM,aAM}   = amosi,
       {eBM,wBM,rBM,iBM,aBM}   = bmosi,
       {eCM,wCM,rCM,iCM,aCM}   = cmosi;

wire  Areq  =  eAM  & (aAM[31:14] == BASE[31:14]), //JLPL - 8_25_25
      Breq  =  eBM  & (aBM[31:14] == BASE[31:14]), //JLPL - 8_25_25
      Creq  =  eCM  & (aCM[31:14] == BASE[31:14]); //JLPL - 8_25_25

// Arbitrate -- well, give priority to the A side
wire              eDM   =  Areq | Breq  |  Creq | XC;                                     // RAM enable
wire              cDM   =  Areq ? ~wAM  & ~rAM                                            // Decode Exchange (SWAP)
                        :  Breq ? ~wBM  & ~rBM
                        :  Creq ? ~wCM  & ~rCM  : 1'b0;
wire              rDM   =  Areq ? ~wAM  &  rAM                                            // Decode Read and Clear
                        :  Breq ? ~wBM  &  rBM
                        :  Creq ? ~wCM  &  rCM  : 1'b0;
wire              wDM   =  Areq ?  wAM  & ~rAM                                            // Decode Read and Clear
                        :  Breq ?  wBM  & ~rBM
                        :  Creq ?  wCM  & ~rCM  : 1'b0;
wire              xDM   =  Areq ?  wAM  &  rAM                                            // Decode Read and Clear
                        :  Breq ?  wBM  &  rBM
                        :  Creq ?  wCM  &  rCM  : 1'b0;
wire  [WIDTH-1:0] iDM   =  cDM          ?  32'h0
                        :  Areq || XCA  ?  iAM
                        :  Breq || XCB  ?  iBM
                        :  Creq || XCC  ?  iCM  : 32'h0;

wire  [     11:0] aDM   =  Areq || XCA  ?  aAM[13:2]             //JLPL - 9_4_25
                        :  Breq || XCB  ?  aBM[13:2]             //JLPL - 9_4_25
                        :  Creq || XCC  ?  aCM[13:2] : 12'h0;    //JLPL - 9_4_25
//
wire [2*WIDTH-1:0] mDMn = aDM[0] ? 64'h0000_0000_ffff_ffff : 64'hffff_ffff_0000_0000; //JLPL - 8_25_25
wire  eDMn  =  ~eDM; //JLPL - 8_25_25
wire  wDMn  =  ~wDM; //JLPL - 8_25_25

reg   [63:0] DMo;
wire  clock_SRAM; //JLPL - 11_5_25
`ifdef synthesis
PREICG_X2N_A7P5PP84TR_C14 iCGxx ( .CK(clock), .E(eDM), .SE(1'b0), .ECK(clock_DM) );
SRAM_2KX64M8B2WM iDRAM (.Q(DMo), .CLK(clock_DM), .CEN(eDMn), .A(aDM[11:1]), .D({2{iDM}}), .GWEN(wDMn),
                        .WEN(mDMn), .EMA(3'b011), .EMAW(2'b0), .EMAS(1'b0), .STOV(1'b0),
                        .RET1N(1'b1), .WABL(1'b0), .WABLM(2'b0));
`else

integer  i;
reg   [63:0] RAM [0:2047];

//JLPL - 11_5_25
   preicg iCG (
      .ECK  (clock_SRAM),
      .CK   (clock),
      .E    (eDM),
      .SE   (1'b0)
      );
//End JLPL - 11_5_25

//JLPL - 8_25_25 - Change to Mask RAM compatible format
always @(posedge clock_SRAM) //JLPL - 11_5_25
   if (!eDMn) begin
      DMo   <=  `tCQ RAM[aDM[11:1]];
      if (!wDMn)
         for (i=0; i<64; i=i+1)
            if (!mDMn[i])
               RAM[aDM[11:1]][i] <= `tCQ  iDM[i%32];
      end
`endif

always @(posedge clock) XC    <= `tCQ  xDM  |  cDM; //JLPL - 8_24_25
always @(posedge clock) XCA   <= `tCQ (wAM  ^~ rAM) & Areq; //JLPL - 8_25_25
always @(posedge clock) XCB   <= `tCQ (wBM  ^~ rBM) & Breq; //JLPL - 8_25_25
always @(posedge clock) XCC   <= `tCQ (wCM  ^~ rCM) & Creq; //JLPL - 8_25_25
always @(posedge clock) AMk   <= `tCQ  Areq;                //JLPL - 8_25_25
always @(posedge clock) BMk   <= `tCQ ~Areq &  Breq;        //JLPL - 8_22_25
always @(posedge clock) CMk   <= `tCQ ~Areq & ~Breq & Creq; //JLPL - 8_22_25
always @(posedge clock) ADz   <= `tCQ  aDM[0];              //JLPL - 8_25_25

wire  [31:0] half =  ADz ? DMo[63:32] : DMo[31:0];

assign   amiso =  {~XC  & Areq,                 {2*WIDTH+1{AMk}} & {1'b1, DMo }},  // XC takes the grant away
         bmiso =  {~(XC | Areq) & Breq ,        {  WIDTH+1{BMk}} & {1'b1, half}},  // XC or Areq takes the grant away
         cmiso =  {~(XC | Areq  | Breq) & Creq, {  WIDTH+1{CMk}} & {1'b1, half}};  // XC or Areq or Breq takes the grant away

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
