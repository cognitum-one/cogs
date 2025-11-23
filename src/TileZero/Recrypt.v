/* ******************************************************************************************************************************

   Copyright © 2024 Advanced Architectures

   All rights reserved
   Confidential Information
   Limited Distribution to Authorized Persons Only
   Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

   Project Name         : Newport

   Description          : TileZero -- Recrypt module for passing thorugh NEWS network

*********************************************************************************************************************************
   THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
   IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
   BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
   TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
   AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
   ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*********************************************************************************************************************************


****************************************************************************************************************************** */

module Recrypt (
   input  wire          clock,  //Joel
   input  wire          reset,  //Joel
   // AES Encryption Block Input Interface
   input  wire [40:0]   amiso,      // AES output   41 {ECBor, ECBad[3:0], ECBpo[3:0], ECB[31:0]} NB ir belongs to nmosi //JLSW
   output wire          amiso_rd,   // Read / Take  (Selected destination is ready!)
   // AES Request ECB Interface
   output wire [ 2:0]   rmosi,      // AES requests
   output wire          rmosi_wr,
   input  wire          rmosi_ir,
   output wire          parity_error, //JLPL - 11_8_25
   // NEWS Pipes
   output wire [31:0]   FNDBo,      // From North Decrypt Block out to NEWS
   output wire          FN_or,
   input  wire          FN_rd,
   output wire [31:0]   FEDBo,      // From East  Decrypt Block out to NEWS
   output wire          FE_or,
   input  wire          FE_rd,
   output wire [31:0]   FWDBo,      // From West  Decrypt Block out to NEWS
   output wire          FW_or,
   input  wire          FW_rd,
   output wire [31:0]   FSDBo,      // From South Decrypt Block out to NEWS
   output wire          FS_or,
   input  wire          FS_rd,
   output wire [31:0]   TNEBo,      // To   North Encrypt Block out to NEWS
   output wire          TN_or,
   input  wire          TN_rd,
   output wire [31:0]   TEEBo,      // To   East  Encrypt Block out to NEWS
   output wire          TE_or,
   input  wire          TE_rd,
   output wire [31:0]   TWEBo,      // To   West  Encrypt Block out to NEWS
   output wire          TW_or,
   input  wire          TW_rd,
   output wire [31:0]   TSEBo,      // To   South Encrypt Block out to NEWS
   output wire          TS_or,
   input  wire          TS_rd
   );

// Locals =======================================================================================================================
// amiso
wire           ECBor;
wire  [ 4:0]   ECBad;
wire  [ 3:0]   ECBpi;
wire  [31:0]   ECBi;
// pipes
wire           FNwr, FEwr, FWwr, FSwr, TNwr, TEwr, TWwr, TSwr; //Joel
//wire           FNrd, FErd, FWrd, FSrd, TNrd, TErd, TWrd, TSrd; //Joel
wire           FNir, FEir, FWir, FSir, TNir, TEir, TWir, TSir;
//wire           FNor, FEor, FWor, FSor, TNor, TEor, TWor, TSor; //Joel
wire  [35:0]   FNDBo_int,FEDBo_int,FWDBo_int,FSDBo_int,TNEBo_int,TEEBo_int,TWEBo_int,TSEBo_int; //Joel
wire [35:0] ECBi_int;

assign FNDBo = FNDBo_int[31:0]; //Joel
assign FEDBo = FEDBo_int[31:0]; //Joel
assign FWDBo = FWDBo_int[31:0]; //Joel
assign FSDBo = FSDBo_int[31:0]; //Joel
assign TNEBo = TNEBo_int[31:0]; //Joel
assign TEEBo = TEEBo_int[31:0]; //Joel
assign TWEBo = TWEBo_int[31:0]; //Joel
assign TSEBo = TSEBo_int[31:0]; //Joel
assign ECBi_int = {ECBpi,ECBi}; //Joel


// ==============================================================================================================================
// Decollate amiso
assign {ECBor,ECBad[3:0],ECBpi[3:0],ECBi[31:0]} = amiso[40:0]; //JLSW


assign  FNwr      =  ECBor & (ECBad[3:0] == 4'b0100) & FNir,      // From North write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        FEwr      =  ECBor & (ECBad[3:0] == 4'b0110) & FEir,      // From East  write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        FWwr      =  ECBor & (ECBad[3:0] == 4'b0111) & FWir,      // From West  write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        FSwr      =  ECBor & (ECBad[3:0] == 4'b0101) & FSir,      // From South write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        TNwr      =  ECBor & (ECBad[3:0] == 4'b0000) & TNir,      //  To  North write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        TEwr      =  ECBor & (ECBad[3:0] == 4'b0010) & TEir,      //  To  East  write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        TWwr      =  ECBor & (ECBad[3:0] == 4'b0011) & TWir,      //  To  West  write //JLPL - 8_14_25 Fix Sequence Number to PUF Order
        TSwr      =  ECBor & (ECBad[3:0] == 4'b0001) & TSir;      //  To  South write //JLPL - 8_14_25 Fix Sequence Number to PUF Order

assign  amiso_rd  =  FNwr & FNir | FEwr & FEir | FWwr & FWir | FSwr & FSir //Joel
                  |  TNwr & TNir | TEwr & TEir | TWwr & TWir | TSwr & TSir; //Joel

//                    <-------------OUTPUT-------------> <-------------INPUT------------->
A2_pipe_141 #(36) iFN (.pdo(FNDBo_int),.por(FN_or),.prd(FN_rd), .pir(FNir),.pdi(ECBi_int),.pwr(FNwr), .clock(clock),.reset(reset)); //Joel
A2_pipe_141 #(36) iFE (.pdo(FEDBo_int),.por(FE_or),.prd(FE_rd), .pir(FEir),.pdi(ECBi_int),.pwr(FEwr), .clock(clock),.reset(reset)); //Joel
A2_pipe_141 #(36) iFW (.pdo(FWDBo_int),.por(FW_or),.prd(FW_rd), .pir(FWir),.pdi(ECBi_int),.pwr(FWwr), .clock(clock),.reset(reset)); //Joel
A2_pipe_141 #(36) iFS (.pdo(FSDBo_int),.por(FS_or),.prd(FS_rd), .pir(FSir),.pdi(ECBi_int),.pwr(FSwr), .clock(clock),.reset(reset)); //Joel

A2_pipe_141 #(36) iTN (.pdo(TNEBo_int),.por(TN_or),.prd(TN_rd), .pir(TNir),.pdi(ECBi_int),.pwr(TNwr), .clock(clock),.reset(reset)); //Joel
A2_pipe_141 #(36) iTE (.pdo(TEEBo_int),.por(TE_or),.prd(TE_rd), .pir(TEir),.pdi(ECBi_int),.pwr(TEwr), .clock(clock),.reset(reset)); //Joel
A2_pipe_141 #(36) iTW (.pdo(TWEBo_int),.por(TW_or),.prd(TW_rd), .pir(TWir),.pdi(ECBi_int),.pwr(TWwr), .clock(clock),.reset(reset)); //Joel
A2_pipe_141 #(36) iTS (.pdo(TSEBo_int),.por(TS_or),.prd(TS_rd), .pir(TSir),.pdi(ECBi_int),.pwr(TSwr), .clock(clock),.reset(reset)); //Joel

wire parity_check; //Joel

assign parity_check = (FNDBo_int[35:32] == {~^FNDBo_int[31:24], ~^FNDBo_int[23:16], ~^FNDBo_int[15:8], ~^FNDBo_int[7:0]}) //Joel
             & (FEDBo_int[35:32] == {~^FEDBo_int[31:24], ~^FEDBo_int[23:16], ~^FEDBo_int[15:8], ~^FEDBo_int[7:0]}) //Joel
             & (FWDBo_int[35:32] == {~^FWDBo_int[31:24], ~^FWDBo_int[23:16], ~^FWDBo_int[15:8], ~^FWDBo_int[7:0]}) //Joel
             & (FSDBo_int[35:32] == {~^FSDBo_int[31:24], ~^FSDBo_int[23:16], ~^FSDBo_int[15:8], ~^FSDBo_int[7:0]}) //Joel
             & (TNEBo_int[35:32] == {~^TNEBo_int[31:24], ~^TNEBo_int[23:16], ~^TNEBo_int[15:8], ~^TNEBo_int[7:0]}) //Joel
             & (TEEBo_int[35:32] == {~^TEEBo_int[31:24], ~^TEEBo_int[23:16], ~^TEEBo_int[15:8], ~^TEEBo_int[7:0]}) //Joel
             & (TWEBo_int[35:32] == {~^TWEBo_int[31:24], ~^TWEBo_int[23:16], ~^TWEBo_int[15:8], ~^TWEBo_int[7:0]}) //Joel
             & (TSEBo_int[35:32] == {~^TSEBo_int[31:24], ~^TSEBo_int[23:16], ~^TSEBo_int[15:8], ~^TSEBo_int[7:0]}); //Joel

assign parity_error = ~parity_check; //JLPL - 11_8_25

//JLPL - 8_12_25
reg [1:0] req_FN_state = 2'b00;
reg FN_request = 1'b0;
reg [1:0] req_FS_state = 2'b00;
reg FS_request = 1'b0;
reg [1:0] req_FE_state = 2'b00;
reg FE_request = 1'b0;
reg [1:0] req_FW_state = 2'b00;
reg FW_request = 1'b0;
reg [1:0] req_TN_state = 2'b00;
reg TN_request = 1'b0;
reg [1:0] req_TS_state = 2'b00;
reg TS_request = 1'b0;
reg [1:0] req_TE_state = 2'b00;
reg TE_request = 1'b0;
reg [1:0] req_TW_state = 2'b00;
reg TW_request = 1'b0;

always @(posedge clock)
	begin
		if ( reset )
			begin
				req_FN_state <= 2'b00;
				req_FS_state <= 2'b00;
				req_FE_state <= 2'b00;
				req_FW_state <= 2'b00;
				req_TN_state <= 2'b00;
				req_TS_state <= 2'b00;
				req_TE_state <= 2'b00;
				req_TW_state <= 2'b00;
				FN_request <= 1'b0;
				FS_request <= 1'b0;
				FE_request <= 1'b0;
				FW_request <= 1'b0;
				TN_request <= 1'b0;
				TS_request <= 1'b0;
				TE_request <= 1'b0;
				TW_request <= 1'b0;
			end //end of if reset
		else
			begin
				if ( req_FN_state == 2'b00 && FNir )
					begin
						req_FN_state <= 2'b01;
						FN_request <= 1'b1;
					end //end of FN_state == 00
				if ( req_FN_state == 2'b01 && rmosi == 3'b100 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_FN_state <= 2'b10;
						FN_request <= 1'b0;
					end //end of FN_state == 01
				if ( req_FN_state == 2'b10 && FNwr )
					req_FN_state <= 2'b11;
				if ( req_FN_state == 2'b11 && !FNwr )
					req_FN_state <= 2'b00;	

				if ( req_FE_state == 2'b00 && FEir )
					begin
						req_FE_state <= 2'b01;
						FE_request <= 1'b1;
					end //end of FE_state == 00
				if ( req_FE_state == 2'b01 && rmosi == 3'b110 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_FE_state <= 2'b10;
						FE_request <= 1'b0;
					end //end of FE_state == 01
				if ( req_FE_state == 2'b10 && FEwr )
					req_FE_state <= 2'b11;
				if ( req_FE_state == 2'b11 && !FEwr )
					req_FE_state <= 2'b00;

				if ( req_FW_state == 2'b00 && FWir )
					begin
						req_FW_state <= 2'b01;
						FW_request <= 1'b1;
					end //end of FW_state == 00
				if ( req_FW_state == 2'b01 && rmosi == 3'b111 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_FW_state <= 2'b10;
						FW_request <= 1'b0;
					end //end of FW_state == 01
				if ( req_FW_state == 2'b10 && FWwr )
					req_FW_state <= 2'b11;
				if ( req_FW_state == 2'b11 && !FWwr )
					req_FW_state <= 2'b00;	

				if ( req_FS_state == 2'b00 && FSir )
					begin
						req_FS_state <= 2'b01;
						FS_request <= 1'b1;
					end //end of FS_state == 00
				if ( req_FS_state == 2'b01 && rmosi == 3'b101 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order 
					begin
						req_FS_state <= 2'b10;
						FS_request <= 1'b0;
					end //end of FS_state == 01
				if ( req_FS_state == 2'b10 && FSwr )
					req_FS_state <= 2'b11;
				if ( req_FS_state == 2'b11 && !FSwr )
					req_FS_state <= 2'b00;

				if ( req_TN_state == 2'b00 && TNir )
					begin
						req_TN_state <= 2'b01;
						TN_request <= 1'b1;
					end //end of TN_state == 00
				if ( req_TN_state == 2'b01 && rmosi == 3'b000 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_TN_state <= 2'b10;
						TN_request <= 1'b0;
					end //end of TN_state == 01
				if ( req_TN_state == 2'b10 && TNwr )
					req_TN_state <= 2'b11;
				if ( req_TN_state == 2'b11 && !TNwr )
					req_TN_state <= 2'b00;

				if ( req_TS_state == 2'b00 && TSir )
					begin
						req_TS_state <= 2'b01;
						TS_request <= 1'b1;
					end //end of TS_state == 00
				if ( req_TS_state == 2'b01 && rmosi == 3'b001 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_TS_state <= 2'b10;
						TS_request <= 1'b0;
					end //end of TS_state == 01
				if ( req_TS_state == 2'b10 && TSwr )
					req_TS_state <= 2'b11;
				if ( req_TS_state == 2'b11 && !TSwr )
					req_TS_state <= 2'b00;	

				if ( req_TE_state == 2'b00 && TEir )
					begin
						req_TE_state <= 2'b01;
						TE_request <= 1'b1;
					end //end of TE_state == 00
				if ( req_TE_state == 2'b01 && rmosi == 3'b010 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_TE_state <= 2'b10;
						TE_request <= 1'b0;
					end //end of TE_state == 01
				if ( req_TE_state == 2'b10 && TEwr )
					req_TE_state <= 2'b11;
				if ( req_TE_state == 2'b11 && !TEwr )
					req_TE_state <= 2'b00;

				if ( req_TW_state == 2'b00 && TWir )
					begin
						req_TW_state <= 2'b01;
						TW_request <= 1'b1;
					end //end of TW_state == 00
				if ( req_TW_state == 2'b01 && rmosi == 3'b011 && rmosi_wr & rmosi_ir ) //JLPL - 8_14_25 - Adjust requests to PUF Key Order
					begin
						req_TW_state <= 2'b10;
						TW_request <= 1'b0;
					end //end of TW_state == 01
				if ( req_TW_state == 2'b10 && TWwr )
					req_TW_state <= 2'b11;
				if ( req_TW_state == 2'b11 && !TWwr )
					req_TW_state <= 2'b00;					
			end //end else reset
	end //end of always
//End JLPL - 8_12_25

//wire [7:0] request = {FNir,FEir,FWir,FSir,TNir,TEir,TWir,TSir}; //Joel //JLPL - 7_31_25 - Temporary change to line up FEwr write above to request bit number
//wire [7:0] request = {FNir,TWir,FWir,FSir,TNir,TEir,FEir,TSir}; //Joel //JLPL - 7_31_25 - Temporary change to line up FEwr write above to request bit number
//wire [7:0] request = {FNir,TWir,TEir,FSir,TNir,FWir,FEir,TSir}; //Joel //JLPL - 8_3_25 - Temporary change to line up FWwr write above to request bit number
//wire [7:0] request = {FNir,TWir,TEir,TNir,FSir,FWir,FEir,TSir}; //Joel //JLPL - 8_4_25 - Temporary change to line up FSwr write above to request bit number
//wire [7:0] request = {TSir,TWir,TEir,TNir,FSir,FWir,FEir,FNir}; //Joel //JLPL - 8_5_25 - Temporary change to line up FNwr write above to request bit number
//wire [7:0] request = {TWir,TEir,TSir,TNir,FSir,FWir,FEir,FNir}; //Joel //JLPL - 8_12_25 - Temporary change to line up TSwr,TEwr and TWwr writes above to request bit number
//wire [7:0] request = {TW_request,TE_request,TS_request,TN_request,FS_request,FW_request,FE_request,FN_request}; //JLPL - 8_14_25 - Adjust requests to PUF Key Order
wire [7:0] request = {FW_request,FE_request,FS_request,FN_request,TW_request,TE_request,TS_request,TN_request}; //JLPL - 8_14_25 - Adjust requests to PUF Key Order
wire [7:0] win; //Joel

A2_square_sparrow #(8) i_arb (
   .win  (win[7:0]),          // The selected winner of arbitration
   .req  (request[7:0]),      // Incoming Requests
   .adv  (rmosi_ir),          // Request taken!  Reload Last Register. Tie to '1' if always taken.
   .en   (1'b1),              // Global Enable:  if false no win!!
   .clock(clock),
   .reset(reset)
   );

assign rmosi_wr =  |win[7:0]; //Joel
assign rmosi    =  {3{win[0]}} & 3'b000   //Joel
                     |  {3{win[1]}} & 3'b001
                     |  {3{win[2]}} & 3'b010
                     |  {3{win[3]}} & 3'b011
                     |  {3{win[4]}} & 3'b100
                     |  {3{win[5]}} & 3'b101
                     |  {3{win[6]}} & 3'b110
                     |  {3{win[7]}} & 3'b111;

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
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
