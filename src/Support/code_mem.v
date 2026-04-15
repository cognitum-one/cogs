//==============================================================================================================================
//
//    Copyright © 2025 Advanced Architectures
//
//    All rights reserved
//    Confidential Information
//    Limited Distribution to authorized Persons Only
//    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.
//
//    Project Name         : 1 read port, 1 read/write port, 1 ROM that auto-downloads to RAM
//
//===============================================================================================================================
//       THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
//    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
//    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
//    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
//    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
//    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//===============================================================================================================================
//    NOTES
//
//===============================================================================================================================

module code_mem #(
   parameter   BYTES = 65536,
   parameter   WIDTH = 32
   )(
   output wire [  WIDTH+1:0]  cmiso,            // {grnt, ack, rdd[31:0]}             -- grant, acknowledge, read-data
   output wire [  WIDTH+1:0]  dmiso,            // {grnt, ack, rdd[31:0]}             -- grant, acknowledge, read-data
   output wire                done,
   input  wire [  WIDTH  :0]  cmosi,            // {en,                    adr[31:0]} -- request,  address
   input  wire [2*WIDTH+2:0]  dmosi,            // {en, wr, rd, wdd[31:0], adr[31:0]} -- request, write, write-data address
   input  wire                boot,
   input  wire                reset,            // reset before boot!!
   input  wire                clock
   );

localparam        DEPTH = BYTES / 4;           // 16384
localparam        LG2D  = 14;                  //    14  Actual Memory Address width

// Bus elements -----------------------------------------------------------------------------------------------------------------
wire  [WIDTH-1:0] cadr,       dadr,   dwrd;
wire              creq, cen,  dreq, den, dwr, drd, active;
reg               CMk,  DMk;
// Boot elements ----------------------------------------------------------------------------------------------------------------
reg   [LG2D -1:0] ADDR;
reg               OFLO;
wire              ldRAM;
// ROM --------------------------------------------------------------------------------------------------------------------------
reg   [WIDTH-1:0] ROMo;
wire              eROMn;
wire  [LG2D -1:0] aROM;
`ifdef synthesis
        ROM_16KX32_M32 i_viaROM (.Q(ROMo), .CLK(clock), .CEN(~eROMn), .STOV(1'b0), .A(aROM),
                                 .TA(), .TQ(), .AY(), .CENY(), .PRDYN(), .EMA(3'b010), .TEN(1'b1),
                                 .BEN(1'b1), .TCEN(1'b1), .PGEN(1'b0), .KEN(1'b1));
`else

reg   [WIDTH-1:0] VIA   [0:DEPTH-1];

always @(posedge  clock)
   if (~eROMn) ROMo <= #1600  VIA[aROM];
`endif

// RAM --------------------------------------------------------------------------------------------------------------------------
reg   [WIDTH-1:0] RAM  [0:DEPTH-1];
reg   [WIDTH-1:0] RAMo;
wire  [WIDTH-1:0] iRAM;
wire  [LG2D -1:0] aRAM;
wire              eRAMn, wRAMn;
wire              clock_SRAM; //JLPL - 11_5_25
`ifdef synthesis

       PREICG_X2N_A7P5PP84TR_C14 iCGxx (.ECK(clock_SRAM), .CK(clock), .E(~eRAMn), .SE(1'b0)); //JLPL - 11_5_25
       SRAM_16KX32M16B2 i_SRAM (.Q(RAMo), .CLK(clock_SRAM), .CEN(eRAMn), .GWEN(wRAMn), .A(aRAM), .D(iRAM),
                           .STOV(1'b0), .EMA(3'b011), .EMAW(2'b00), .EMAS(1'b0), .RET1N(1'b1),
                           .WABL(1'b0), .WABLM(2'b00) );
`else

//JLPL - 11_5_25
   preicg iCG (
      .ECK  (clock_SRAM),
      .CK   (clock),
      .E    (~eRAMn),
      .SE   (1'b0)
      );
//End JLPL - 11_5_25
	  
always @(posedge  clock_SRAM) //JLPL - 11_5_25
   if (~eRAMn) begin
      RAMo  <=  #510  RAM[aRAM];
      if (~wRAMn) RAM[aRAM]  <=  iRAM;
      end
`endif

// FSM for Boot -----------------------------------------------------------------------------------------------------------------

reg   [2:0] FSM; localparam [2:0]  IDLE  = 0,  READ = 1, CYC1 = 2, CYC2 = 3, LOAD = 4, DONE = 5;

always @(posedge clock)
   if (reset)  begin
      FSM   <=  IDLE;
      ADDR  <= {LG2D{1'b0}};
      OFLO  <=  1'b0;
      end
   else case(FSM)
      IDLE  :  if (boot)  FSM <=  READ;
      READ  :             FSM <=  OFLO ? DONE:CYC1;
      CYC1  :             FSM <=  CYC2;
      CYC2  :             FSM <=  LOAD;
      LOAD  :  begin      FSM <=  READ;
                  {OFLO,ADDR} <=  ADDR + 1'b1;
                  end
      default             FSM <=  DONE;
      endcase

assign
   eROMn = ~(FSM==READ),
   ldRAM =   FSM==LOAD;

// Post-boot operation ----------------------------------------------------------------------------------------------------------
assign
   // Decollate input buses
   {cen,                 cadr}  =  cmosi,
   {den, dwr, drd, dwrd, dadr}  =  dmosi,
   // Arbitrate and select
   creq     =   cen  & (cadr[WIDTH-1:LG2D+2] == {WIDTH-LG2D-2{1'b0}}),    // Check upper bits for zero
   dreq     =   den  & (dadr[WIDTH-1:LG2D+2] == {WIDTH-LG2D-2{1'b0}}),
   active   =   OFLO & (creq | dreq),
   eRAMn    = ~(OFLO ? (creq | dreq) : ldRAM),
   wRAMn    = ~(dreq &  dwr  | ldRAM),
   aROM     =           ADDR[LG2D-1:0],
   aRAM     =  !OFLO ?  ADDR[LG2D-1:0]          // Boot is highest priority
            :   creq ?  cadr[LG2D+1:2]          // Yes, the D-port is higher priority than the C-port !!
            :           dadr[LG2D+1:2],
   iRAM     =   OFLO ?  dwrd  :  ROMo;

// Acknowledges -----------------------------------------------------------------------------------------------------------------
always @(posedge clock) if (active || CMk ) CMk <= creq;
always @(posedge clock) if (active || DMk ) DMk <= dreq & ~creq;

// Collate output buses ---------------------------------------------------------------------------------------------------------
assign   //  grant             ack           data
   cmiso = { creq,         {33{CMk}} & {1'b1,RAMo[WIDTH-1:0]}},
   dmiso = {~creq & dreq,  {33{DMk}} & {1'b1,RAMo[WIDTH-1:0]}},
   done  =   OFLO;

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

`ifdef   verify_code_mem

module t;

parameter   WIDTH = 32;

wire [  WIDTH+1:0]  cmiso;             // {grnt, ack, rdd[31:0]}            -- grant, acknowledge, read-data
wire [  WIDTH+1:0]  dmiso;             // {grnt, ack, rdd[31:0]}            -- grant, acknowledge, read-data
wire                done;
reg  [  WIDTH  :0]  cmosi;             // {en,                adr[31:0]}    -- request,  address
reg  [2*WIDTH+1:0]  dmosi;             // {en, wr, wdd[31:0], adr[31:0]}    -- request, write, write-data address
reg                 boot;
reg                 reset;             // reset before boot!!
reg                 clock;

code_mem #(65536,32) mut (
   .cmiso(cmiso),
   .dmiso(dmiso),
   .done (done ),
   .cmosi(cmosi),
   .dmosi(dmosi),
   .boot (boot ),
   .reset(reset),
   .clock(clock)
   );

initial begin
   clock = 1'b0;
   forever
      clock = #1000 ~clock;
   end

integer  i;
reg   [31:0] temp;
initial begin
   for (i=0; i<16384; i=i+1) begin
      temp = {~i[15:0],i[15:0]};
      mut.VIA[i] = temp;
      end
   reset    =  0;
   boot     =  0;
   cmosi    = 33'h0;
   dmosi    = 66'h0;
   repeat (10) @(posedge clock);
   reset    =  1;
   repeat ( 1) @(posedge clock);
   reset    =  0;
   boot     =  1;
   repeat (70000) @(posedge clock);
   if (done)
      boot  =  0;
   repeat ( 1) @(posedge clock);

   cmosi    =  {1'b1, 32'h00000034};
   repeat ( 1) @(posedge clock); #600;

   if (cmiso != {2'b11, 32'hfff2000d}) $display(" Error : Read expected 32'hfff2000d, found %h",cmiso[31:0]);
   cmosi    =  {1'b1, 32'h00000078};
   repeat ( 1) @(posedge clock); #600;

   if (cmiso != {2'b11, 32'hffe1001e}) $display(" Error : Read expected 32'hffe1001e, found %h",cmiso[31:0]);
   cmosi    =  {1'b1, 32'h000000c6};
   repeat ( 1) @(posedge clock); #600;

   if (cmiso != {2'b11, 32'hffce0031}) $display(" Error : Read expected 32'hffce0031, found %h",cmiso[31:0]);
   cmosi    =  {1'b1, 32'h00000003};
   repeat ( 1) @(posedge clock); #600;

   if (cmiso != {2'b11, 32'hffff0000}) $display(" Error : Read expected 32'hffff0000, found %h",cmiso[31:0]);
   cmosi    =  {1'b1, 32'h00000011};
   repeat ( 1) @(posedge clock); #600;

   if (cmiso != {2'b11, 32'hfffb0004}) $display(" Error : Read expected 32'hfffb0004, found %h",cmiso[31:0]);
   cmosi    =  {1'b0, 32'h00000000};
   repeat ( 1) @(posedge clock); #600;

   if (cmiso != {2'b10, 32'hfffb0004}) $display(" Error : Read expected 32'hfffb0004, found %h",cmiso[31:0]);
   repeat (11) @(posedge clock);
   $stop();
   end

endmodule
`endif
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

